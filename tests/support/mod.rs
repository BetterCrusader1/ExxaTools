//! A Steam tree and a Vapor Forge home the tests own, so no test reads the real
//! client. Every path is created under one temporary root that removes itself.

use std::path::{Path, PathBuf};

pub struct Tree {
    pub root: PathBuf,
}

impl Tree {
    pub fn new(tag: &str) -> Tree {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("exxa-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&root).expect("fixture root");
        let tree = Tree { root };
        tree.build();
        tree
    }

    pub fn steam(&self) -> PathBuf {
        self.root.join("steam")
    }

    pub fn vf(&self) -> PathBuf {
        self.root.join("vapor-forge")
    }

    pub fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// Points every path helper at this tree for the rest of the process.
    pub fn apply(&self) {
        std::env::set_var("EXXATOOLS_STEAM_ROOT", self.steam());
        std::env::set_var("EXXATOOLS_VF_HOME", self.vf());
        std::env::set_var("EXXATOOLS_HOME", self.home());
    }

    fn build(&self) {
        let steam = self.steam();
        write(&steam.join("config/loginusers.vdf"), "\"users\"\n{\n\t\"12345\"\n\t{\n\t\t\"PersonaName\"\t\t\"fixture\"\n\t}\n}\n");
        write(&steam.join("steamapps/libraryfolders.vdf"), &format!("\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n", steam.display()));
        write(&steam.join("steamapps/appmanifest_620.acf"), "\"AppState\"\n{\n\t\"appid\"\t\t\"620\"\n\t\"name\"\t\t\"Portal 2\"\n\t\"StateFlags\"\t\t\"4\"\n\t\"BytesDownloaded\"\t\t\"0\"\n\t\"BytesToDownload\"\t\t\"1000\"\n\t\"AutoUpdateBehavior\"\t\t\"0\"\n}\n");
        write(&steam.join("userdata/12345/config/localconfig.vdf"), "\"UserLocalConfigStore\"\n{\n\t\"Software\"\n\t{\n\t\t\"Valve\"\n\t\t{\n\t\t\t\"Steam\"\n\t\t\t{\n\t\t\t\t\"apps\"\n\t\t\t\t{\n\t\t\t\t\t\"620\"\n\t\t\t\t\t{\n\t\t\t\t\t\t\"LaunchOptions\"\t\t\"\"\n\t\t\t\t\t}\n\t\t\t\t}\n\t\t\t}\n\t\t}\n\t}\n}\n");
        write(&self.vf().join("config.toml"), "manifest_url = \"\"\n");
        std::fs::create_dir_all(self.vf().join("scripts")).expect("scripts dir");
        std::fs::create_dir_all(self.home()).expect("state dir");
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("fixture parent");
    }
    std::fs::write(path, text).expect("fixture write");
}
