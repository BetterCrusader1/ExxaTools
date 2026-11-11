mod paths;

fn main() {
    println!("exxa {} at {}", env!("CARGO_PKG_VERSION"), paths::project_dir().display());
    println!("steam: {:?}", paths::steam_root());

    if let Ok(steam) = steamlocate::SteamDir::locate() {
        println!("steamdir: {}", steam.path().display());
        if let Ok(libraries) = steam.libraries() {
            for library in libraries.filter_map(|entry| entry.ok()) {
                println!("  library: {}", library.path().display());
            }
        }
    }

    let acf = r#""AppState"
{
	"appid"		"620"
	"StateFlags"		"4"
}
"#;
    match keyvalues_parser::parse(acf).map(keyvalues_parser::Vdf::from) {
        Ok(vdf) => {
            if let Some(obj) = vdf.value.get_obj() {
                for (key, values) in obj.iter() {
                    let first = values.first().map(|value| value.to_string()).unwrap_or_default();
                    println!("acf: {key} = {first}");
                }
            }
        }
        Err(error) => println!("acf parse failed: {error}"),
    }
}