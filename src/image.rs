use std::path::Path;

use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use error_stack::{Report, ResultExt};

use crate::error::Error;

pub struct ImageData {
    pub mimetype: String,
    pub contents: Vec<u8>,
}

impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageData")
            .field("mimetype", &self.mimetype)
            .finish_non_exhaustive()
    }
}

impl ImageData {
    pub fn new(filename: &Path) -> Result<Self, Report<Error>> {
        let contents = std::fs::read(filename).change_context(Error::Image)?;
        let info = imageinfo::ImageInfo::from_raw_data(&contents).change_context(Error::Image)?;

        Ok(ImageData {
            mimetype: info.mimetype.to_string(),
            contents,
        })
    }

    pub fn as_base64(&self) -> String {
        Base64Display::new(&self.contents, &STANDARD).to_string()
    }

    pub fn as_data_url(&self) -> String {
        format!(
            "data:{};base64,{}",
            self.mimetype,
            Base64Display::new(&self.contents, &STANDARD)
        )
    }
}
