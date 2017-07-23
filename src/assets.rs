use stylish_webrender;

pub struct AssetLoader;

impl stylish_webrender::Assets for AssetLoader {
    fn load_font(&self, name: &str) -> Option<Vec<u8>> {
        use std::fs;
        use std::io::Read;
        let mut f = if let Ok(f) = fs::File::open(format!("fonts/{}.ttf", name)) {
            f
        } else {
            return None;
        };
        let mut data = Vec::new();
        f.read_to_end(&mut data)
            .ok()
            .map(|_| data)
    }
    fn load_image(&self, _name: &str) -> Option<stylish_webrender::Image> {
        None
    }
}