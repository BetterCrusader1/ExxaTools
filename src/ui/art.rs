//! Cover and hero art decoded off the main thread into GPU textures. A
//! byte-bounded LRU keeps scrolling from decoding twice or growing without
//! limit; every widget asks for exactly the pixels it shows at its scale.
//! Backdrops are blurred here on the CPU at a tiny size, so the window-wide
//! game background costs tens of kilobytes and no per-frame GPU blur.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gdk, glib};

/// Decoded art kept resident. Discover at 2x is ~20 MB for 60 covers, so this
/// holds a full page plus the library and a few heroes before evicting.
const BUDGET_BYTES: usize = 48 * 1024 * 1024;

/// Backdrops decode this wide regardless of window size: after the blur the
/// texture is upscaled with linear filtering and still looks smooth.
const BACKDROP_WIDTH: i32 = 160;
/// Box radius at `BACKDROP_WIDTH`; three passes approximate a Gaussian of
/// roughly 50 px once the backdrop is stretched across the window.
const BACKDROP_RADIUS: usize = 5;
/// White text on the accent must stay readable (WCAG AA for normal text).
const ACCENT_MIN_CONTRAST: f64 = 4.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    Plain,
    Backdrop,
    /// Transparent title art, trimmed to its visible pixels so it aligns
    /// with the text beside it.
    Logo,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Key {
    path: PathBuf,
    width: i32,
    height: i32,
    kind: Kind,
}

/// An sRGB color taken from the artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// A decoded texture plus, for backdrops, the game's accent color.
#[derive(Clone)]
pub struct Loaded {
    pub texture: gdk::Texture,
    pub accent: Option<Rgb>,
}

/// Least-recently-used map whose capacity is a byte cost, not an entry count.
struct Lru<K, V> {
    entries: HashMap<K, (V, usize, u64)>,
    bytes: usize,
    budget: usize,
    tick: u64,
}

impl<K: Clone + Eq + Hash, V: Clone> Lru<K, V> {
    fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            budget,
            tick: 0,
        }
    }

    fn get(&mut self, key: &K) -> Option<V> {
        self.tick += 1;
        let tick = self.tick;
        self.entries.get_mut(key).map(|entry| {
            entry.2 = tick;
            entry.0.clone()
        })
    }

    fn insert(&mut self, key: K, value: V, cost: usize) {
        self.tick += 1;
        if let Some((_, old, _)) = self.entries.insert(key, (value, cost, self.tick)) {
            self.bytes -= old;
        }
        self.bytes += cost;
        while self.bytes > self.budget && self.entries.len() > 1 {
            let Some(oldest) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.2)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some((_, cost, _)) = self.entries.remove(&oldest) {
                self.bytes -= cost;
            }
        }
    }
}

type Waiter = Box<dyn FnOnce(Option<Loaded>)>;

struct Store {
    cache: Lru<Key, Loaded>,
    pending: HashMap<Key, Vec<Waiter>>,
}

thread_local! {
    static STORE: RefCell<Store> = RefCell::new(Store {
        cache: Lru::new(BUDGET_BYTES),
        pending: HashMap::new(),
    });
}

/// Raw pixels produced on the worker thread; `glib::Bytes` is `Send`, a
/// `Pixbuf` is not, so the texture itself is built back on the main thread.
struct Decoded {
    bytes: glib::Bytes,
    width: i32,
    height: i32,
    stride: usize,
    alpha: bool,
    accent: Option<Rgb>,
}

/// `height < 0` keeps the source aspect ratio at the requested width.
fn decode(key: &Key) -> Option<Decoded> {
    let preserve = key.height < 0;
    let pixbuf =
        gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(&key.path, key.width, key.height, preserve)
            .ok()?;
    let width = pixbuf.width();
    let height = pixbuf.height();
    let stride = pixbuf.rowstride() as usize;
    if key.kind == Kind::Logo {
        let trimmed = pixbuf
            .has_alpha()
            .then(|| {
                let bytes = pixbuf.read_pixel_bytes();
                alpha_bbox(&bytes, width as usize, height as usize, stride)
            })
            .flatten()
            .and_then(|(x, y, w, h)| {
                pixbuf
                    .new_subpixbuf(x as i32, y as i32, w as i32, h as i32)
                    .copy()
            })
            .unwrap_or(pixbuf);
        return Some(Decoded {
            bytes: trimmed.read_pixel_bytes(),
            width: trimmed.width(),
            height: trimmed.height(),
            stride: trimmed.rowstride() as usize,
            alpha: trimmed.has_alpha(),
            accent: None,
        });
    }
    if key.kind == Kind::Plain {
        return Some(Decoded {
            bytes: pixbuf.read_pixel_bytes(),
            width,
            height,
            stride,
            alpha: pixbuf.has_alpha(),
            accent: None,
        });
    }
    let channels = pixbuf.n_channels() as usize;
    let source = pixbuf.read_pixel_bytes();
    let mut rgb = packed_rgb(&source, width as usize, height as usize, stride, channels);
    let accent = accent(&rgb);
    box_blur_rgb(&mut rgb, width as usize, height as usize, BACKDROP_RADIUS);
    Some(Decoded {
        bytes: glib::Bytes::from_owned(rgb),
        width,
        height,
        stride: width as usize * 3,
        alpha: false,
        accent,
    })
}

/// Bounding box `(x, y, width, height)` of pixels with visible alpha in RGBA
/// rows; `None` when everything is transparent.
fn alpha_bbox(rgba: &[u8], width: usize, height: usize, stride: usize) -> Option<(usize, usize, usize, usize)> {
    let (mut left, mut top, mut right, mut bottom) = (width, height, 0, 0);
    for row in 0..height {
        for column in 0..width {
            if rgba.get(row * stride + column * 4 + 3).is_some_and(|&alpha| alpha > 8) {
                left = left.min(column);
                right = right.max(column);
                top = top.min(row);
                bottom = bottom.max(row);
            }
        }
    }
    (left <= right && top <= bottom).then(|| (left, top, right - left + 1, bottom - top + 1))
}

/// Drops row padding and alpha so the blur works on tight RGB triples.
fn packed_rgb(source: &[u8], width: usize, height: usize, stride: usize, channels: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(width * height * 3);
    for row in 0..height {
        let start = row * stride;
        for column in 0..width {
            let at = start + column * channels;
            match source.get(at..at + 3) {
                Some(pixel) => out.extend_from_slice(pixel),
                None => out.extend_from_slice(&[0, 0, 0]),
            }
        }
    }
    out
}

/// Three passes of a separable box blur (a cheap Gaussian) over packed RGB,
/// clamping at the edges so borders do not darken.
fn box_blur_rgb(rgb: &mut [u8], width: usize, height: usize, radius: usize) {
    if width == 0 || height == 0 || radius == 0 || rgb.len() < width * height * 3 {
        return;
    }
    let mut scratch = vec![0u8; rgb.len()];
    for _ in 0..3 {
        blur_pass(rgb, &mut scratch, width, height, radius, true);
        blur_pass(&scratch, rgb, width, height, radius, false);
    }
}

fn blur_pass(
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
    horizontal: bool,
) {
    let (lines, length) = if horizontal { (height, width) } else { (width, height) };
    let at = |line: usize, index: usize| {
        if horizontal {
            (line * width + index) * 3
        } else {
            (index * width + line) * 3
        }
    };
    let window = (2 * radius + 1) as u32;
    for line in 0..lines {
        let mut sum = [0u32; 3];
        for offset in 0..window as usize {
            let index = offset.saturating_sub(radius).min(length - 1);
            let pixel = at(line, index);
            for channel in 0..3 {
                sum[channel] += src[pixel + channel] as u32;
            }
        }
        for index in 0..length {
            let pixel = at(line, index);
            for channel in 0..3 {
                dst[pixel + channel] = ((sum[channel] + window / 2) / window) as u8;
            }
            let leaving = at(line, index.saturating_sub(radius));
            let entering = at(line, (index + radius + 1).min(length - 1));
            for channel in 0..3 {
                sum[channel] = sum[channel] + src[entering + channel] as u32
                    - src[leaving + channel] as u32;
            }
        }
    }
}

/// The artwork's dominant vivid hue, made dark enough for white text. Gray or
/// nearly black art returns `None` so the system accent stays in charge.
fn accent(rgb: &[u8]) -> Option<Rgb> {
    const BUCKETS: usize = 24;
    let mut weight = [0f64; BUCKETS];
    let mut sums = [[0f64; 3]; BUCKETS];
    let mut pixels = 0usize;
    for pixel in rgb.chunks_exact(3) {
        pixels += 1;
        let (hue, saturation, value) = hsv(pixel[0], pixel[1], pixel[2]);
        if saturation < 0.25 || value < 0.2 {
            continue;
        }
        let bucket = ((hue / 360.0) * BUCKETS as f64) as usize % BUCKETS;
        let w = saturation * value;
        weight[bucket] += w;
        for channel in 0..3 {
            sums[bucket][channel] += pixel[channel] as f64 * w;
        }
    }
    let (best, &best_weight) = weight
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    // Fewer than ~4% of the pixels at full vividness: there is no real hue.
    if pixels == 0 || best_weight < pixels as f64 * 0.04 {
        return None;
    }
    let mean = sums[best].map(|sum| sum / best_weight);
    Some(legible(Rgb(
        mean[0].round() as u8,
        mean[1].round() as u8,
        mean[2].round() as u8,
    )))
}

/// Keeps the hue, lifts dull colors to a usable saturation and lowers
/// lightness until white text reaches `ACCENT_MIN_CONTRAST`.
fn legible(color: Rgb) -> Rgb {
    let (hue, saturation, lightness) = hsl(color);
    let saturation = saturation.clamp(0.5, 0.9);
    let mut lightness = lightness.clamp(0.2, 0.6);
    loop {
        let candidate = from_hsl(hue, saturation, lightness);
        if contrast_with_white(candidate) >= ACCENT_MIN_CONTRAST || lightness <= 0.1 {
            return candidate;
        }
        lightness -= 0.01;
    }
}

fn hsv(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    (hue, saturation, max)
}

fn hsl(color: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (
        color.0 as f64 / 255.0,
        color.1 as f64 / 255.0,
        color.2 as f64 / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    let saturation = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * lightness - 1.0).abs())
    };
    (hsv(color.0, color.1, color.2).0, saturation, lightness)
}

fn from_hsl(hue: f64, saturation: f64, lightness: f64) -> Rgb {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = lightness - chroma / 2.0;
    let (r, g, b) = match hue as u32 {
        0..=59 => (chroma, x, 0.0),
        60..=119 => (x, chroma, 0.0),
        120..=179 => (0.0, chroma, x),
        180..=239 => (0.0, x, chroma),
        240..=299 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let channel = |value: f64| ((value + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(channel(r), channel(g), channel(b))
}

fn relative_luminance(color: Rgb) -> f64 {
    let linear = |channel: u8| {
        let value = channel as f64 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.0) + 0.7152 * linear(color.1) + 0.0722 * linear(color.2)
}

fn contrast_with_white(color: Rgb) -> f64 {
    1.05 / (relative_luminance(color) + 0.05)
}

fn texture(decoded: Decoded) -> gdk::Texture {
    let format = if decoded.alpha {
        gdk::MemoryFormat::R8g8b8a8
    } else {
        gdk::MemoryFormat::R8g8b8
    };
    gdk::MemoryTexture::new(
        decoded.width,
        decoded.height,
        format,
        &decoded.bytes,
        decoded.stride,
    )
    .upcast()
}

/// Loads the blurred backdrop for `path` together with its accent color.
/// `done` runs synchronously on a cache hit.
pub fn backdrop(path: &Path, done: impl FnOnce(Option<Loaded>) + 'static) {
    load(
        Key {
            path: path.to_path_buf(),
            width: BACKDROP_WIDTH,
            height: -1,
            kind: Kind::Backdrop,
        },
        Box::new(done),
    );
}

/// Loads a title logo `width` device pixels wide, trimmed to its visible
/// pixels. `done` runs synchronously on a cache hit.
pub fn logo(path: &Path, width: i32, done: impl FnOnce(Option<Loaded>) + 'static) {
    load(
        Key {
            path: path.to_path_buf(),
            width: width.max(1),
            height: -1,
            kind: Kind::Logo,
        },
        Box::new(done),
    );
}

/// Device scale for `widget`, falling back to the densest monitor before it
/// is attached.
pub fn scale(widget: &impl IsA<gtk::Widget>) -> i32 {
    scale_for(widget.upcast_ref())
}

/// Calls `done` synchronously on a cache hit, otherwise once the shared decode
/// finishes. Concurrent requests for the same art share one decode.
fn load(key: Key, done: Waiter) {
    let hit = STORE.with(|store| store.borrow_mut().cache.get(&key));
    if let Some(loaded) = hit {
        done(Some(loaded));
        return;
    }
    let first = STORE.with(|store| {
        let mut store = store.borrow_mut();
        let waiters = store.pending.entry(key.clone()).or_default();
        waiters.push(done);
        waiters.len() == 1
    });
    if !first {
        return;
    }
    DECODER.with(|decoder| decoder.send(key).ok());
}

/// Decoding runs on one long-lived thread instead of the shared GIO pool:
/// every pool thread grows its own malloc arena, which kept several MB of
/// freed JPEG buffers resident for the life of the window.
fn spawn_decoder() -> std::sync::mpsc::Sender<Key> {
    let (tx, rx) = std::sync::mpsc::channel::<Key>();
    std::thread::Builder::new()
        .name("exxa-art".into())
        .spawn(move || {
            for key in rx {
                let decoded = decode(&key);
                glib::idle_add_once(move || finish(key, decoded));
            }
        })
        .expect("spawn art decoder");
    tx
}

thread_local! {
    static DECODER: std::sync::mpsc::Sender<Key> = spawn_decoder();
}

fn finish(key: Key, decoded: Option<Decoded>) {
    let loaded = decoded.map(|decoded| {
        let cost = decoded.stride * decoded.height as usize;
        let accent = decoded.accent;
        let loaded = Loaded {
            texture: texture(decoded),
            accent,
        };
        STORE.with(|store| {
            store
                .borrow_mut()
                .cache
                .insert(key.clone(), loaded.clone(), cost)
        });
        loaded
    });
    let waiters = STORE.with(|store| store.borrow_mut().pending.remove(&key));
    for waiter in waiters.unwrap_or_default() {
        waiter(loaded.clone());
    }
}

/// A picture slot with a skeleton while loading, a short fade when art
/// arrives and a monogram when there is none. Rebinding to another game
/// cancels the stale result, so recycled grid cells never flash wrong art.
#[derive(Clone)]
pub struct Art {
    pub root: gtk::Overlay,
    picture: gtk::Picture,
    letter: gtk::Label,
    width: i32,
    height: i32,
    generation: Rc<Cell<u64>>,
}

impl Art {
    /// `height < 0` lets the picture size itself and keeps the source aspect.
    pub fn new(width: i32, height: i32, class: &str) -> Self {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);

        let letter = gtk::Label::new(None);
        letter.add_css_class("art-letter");
        letter.set_halign(gtk::Align::Center);
        letter.set_valign(gtk::Align::Center);
        letter.set_visible(false);
        letter.set_can_target(false);

        let root = gtk::Overlay::new();
        root.add_css_class("art");
        root.add_css_class(class);
        root.set_overflow(gtk::Overflow::Hidden);
        root.set_size_request(if width > 0 { width } else { -1 }, height.max(-1));
        root.set_child(Some(&picture));
        root.add_overlay(&letter);

        Self {
            root,
            picture,
            letter,
            width,
            height,
            generation: Rc::new(Cell::new(0)),
        }
    }

    /// `decode_width` is the logical width to decode at when the slot itself
    /// stretches (the hero); fixed slots decode at their own size.
    pub fn set(&self, path: Option<&Path>, initial: &str, decode_width: Option<i32>) {
        let generation = self.generation.get().wrapping_add(1);
        self.generation.set(generation);
        self.picture.set_paintable(None::<&gdk::Paintable>);
        self.root.remove_css_class("ready");
        self.root.remove_css_class("instant");
        self.root.remove_css_class("missing");
        self.letter.set_text(initial);
        self.letter.set_visible(false);

        let Some(path) = path else {
            self.show_missing();
            return;
        };
        let scale = scale_for(self.root.upcast_ref());
        let width = decode_width.unwrap_or(self.width).max(1) * scale;
        let height = if decode_width.is_some() || self.height < 0 {
            -1
        } else {
            self.height * scale
        };
        let key = Key {
            path: path.to_path_buf(),
            width,
            height,
            kind: Kind::Plain,
        };

        let this = self.clone();
        let sync = Rc::new(Cell::new(true));
        let in_call = sync.clone();
        load(
            key,
            Box::new(move |loaded| {
                if this.generation.get() != generation {
                    return;
                }
                match loaded {
                    Some(loaded) => {
                        this.picture.set_paintable(Some(&loaded.texture));
                        if in_call.get() {
                            this.root.add_css_class("instant");
                        }
                        this.root.add_css_class("ready");
                    }
                    None => this.show_missing(),
                }
            }),
        );
        sync.set(false);
    }

    /// Posters fill their slot; a wide header shown in a poster slot is
    /// contained instead so its title art is not cropped away.
    pub fn set_content_fit(&self, fit: gtk::ContentFit) {
        self.picture.set_content_fit(fit);
    }

    fn show_missing(&self) {
        self.root.add_css_class("missing");
        self.letter.set_visible(true);
    }
}

/// Widgets built before they are attached report scale 1, which would decode
/// blurry art on HiDPI; fall back to the densest monitor in that case.
fn scale_for(widget: &gtk::Widget) -> i32 {
    if widget.root().is_some() {
        return widget.scale_factor().max(1);
    }
    let Some(display) = gdk::Display::default() else {
        return 1;
    };
    let monitors = display.monitors();
    (0..monitors.n_items())
        .filter_map(|index| monitors.item(index).and_downcast::<gdk::Monitor>())
        .map(|monitor| monitor.scale_factor())
        .max()
        .unwrap_or(1)
        .max(1)
}

pub const STYLE: &str = r#"
.art { background-color: alpha(@window_fg_color, 0.06); }
.art > picture { opacity: 0; }
.art.ready > picture {
    opacity: 1;
    transition: opacity 220ms ease-out;
}
.art.ready.instant > picture { transition: none; }
.art.missing {
    background-color: @card_bg_color;
    box-shadow: inset 0 0 0 1px @borders;
}
.art-letter {
    font-weight: 700;
    color: @insensitive_fg_color;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(color: Rgb, pixels: usize) -> Vec<u8> {
        std::iter::repeat_n([color.0, color.1, color.2], pixels)
            .flatten()
            .collect()
    }

    #[test]
    fn blur_keeps_a_uniform_image_uniform() {
        let mut rgb = solid(Rgb(40, 120, 200), 16 * 9);
        box_blur_rgb(&mut rgb, 16, 9, 3);
        assert!(rgb.chunks_exact(3).all(|pixel| pixel == [40, 120, 200]));
    }

    #[test]
    fn blur_spreads_a_bright_point_without_inventing_energy() {
        let (width, height) = (21, 21);
        let mut rgb = vec![0u8; width * height * 3];
        let center = (10 * width + 10) * 3;
        rgb[center..center + 3].copy_from_slice(&[255, 255, 255]);
        box_blur_rgb(&mut rgb, width, height, 2);
        assert!(rgb[center] < 255, "the point must spread");
        let neighbour = (10 * width + 12) * 3;
        assert!(rgb[neighbour] > 0, "neighbours must pick it up");
        let total: u32 = rgb.iter().step_by(3).map(|&value| value as u32).sum();
        assert!(total <= 255 + (width * height) as u32 / 2, "total {total}");
    }

    #[test]
    fn packing_drops_alpha_and_row_padding() {
        let source = [1, 2, 3, 255, 4, 5, 6, 255, 0, 0, 7, 8, 9, 255, 10, 11, 12, 255, 0, 0];
        assert_eq!(
            packed_rgb(&source, 2, 2, 10, 4),
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
        );
    }

    #[test]
    fn logo_trim_finds_the_visible_pixels() {
        let (width, height) = (6, 4);
        let stride = width * 4 + 2;
        let mut rgba = vec![0u8; stride * height];
        for (x, y) in [(2, 1), (4, 2)] {
            rgba[y * stride + x * 4 + 3] = 255;
        }
        assert_eq!(alpha_bbox(&rgba, width, height, stride), Some((2, 1, 3, 2)));
        assert_eq!(alpha_bbox(&vec![0u8; stride * height], width, height, stride), None);
    }

    #[test]
    fn gray_art_leaves_the_system_accent_alone() {
        assert_eq!(accent(&solid(Rgb(128, 128, 128), 400)), None);
        assert_eq!(accent(&solid(Rgb(8, 6, 10), 400)), None);
    }

    #[test]
    fn a_red_poster_gives_a_readable_red_accent() {
        let color = accent(&solid(Rgb(220, 30, 40), 400)).expect("accent");
        assert!(color.0 > color.1 * 2 && color.0 > color.2 * 2, "{color:?}");
        assert!(contrast_with_white(color) >= ACCENT_MIN_CONTRAST);
    }

    #[test]
    fn a_bright_yellow_is_darkened_but_keeps_its_hue() {
        let color = accent(&solid(Rgb(250, 220, 40), 400)).expect("accent");
        assert!(contrast_with_white(color) >= ACCENT_MIN_CONTRAST, "{color:?}");
        assert!(color.0 > color.2 && color.1 > color.2, "still yellow: {color:?}");
    }

    #[test]
    fn the_dominant_hue_wins_over_a_gray_majority() {
        let mut rgb = solid(Rgb(90, 90, 90), 700);
        rgb.extend(solid(Rgb(30, 90, 230), 300));
        let color = accent(&rgb).expect("accent");
        assert!(color.2 > color.0 && color.2 > color.1, "{color:?}");
    }

    #[test]
    fn contrast_matches_the_wcag_reference_points() {
        assert!((contrast_with_white(Rgb(0, 0, 0)) - 21.0).abs() < 0.01);
        assert!((contrast_with_white(Rgb(255, 255, 255)) - 1.0).abs() < 0.01);
        assert!((contrast_with_white(Rgb(0x35, 0x84, 0xe4)) - 3.77).abs() < 0.02);
    }

    #[test]
    fn eviction_drops_the_least_recently_used_entry_first() {
        let mut lru = Lru::new(10);
        lru.insert("a", 1, 4);
        lru.insert("b", 2, 4);
        assert_eq!(lru.get(&"a"), Some(1));
        lru.insert("c", 3, 4);
        assert_eq!(lru.get(&"b"), None, "b was the least recently used");
        assert_eq!(lru.get(&"a"), Some(1));
        assert_eq!(lru.get(&"c"), Some(3));
        assert_eq!(lru.bytes, 8);
    }

    #[test]
    fn replacing_a_key_does_not_double_count_its_cost() {
        let mut lru = Lru::new(100);
        lru.insert("a", 1, 30);
        lru.insert("a", 2, 50);
        assert_eq!(lru.bytes, 50);
        assert_eq!(lru.get(&"a"), Some(2));
    }

    #[test]
    fn a_single_oversized_entry_is_kept_rather_than_thrashed() {
        let mut lru = Lru::new(10);
        lru.insert("big", 1, 64);
        assert_eq!(lru.get(&"big"), Some(1));
        lru.insert("next", 2, 4);
        assert_eq!(lru.get(&"big"), None);
        assert_eq!(lru.get(&"next"), Some(2));
    }
}
