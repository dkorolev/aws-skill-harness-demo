use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(crate) struct Items<T> {
    #[serde(rename = "item", default)]
    pub items: Vec<T>,
}

impl<T> Default for Items<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(crate) struct Members<T> {
    #[serde(rename = "member", default)]
    pub members: Vec<T>,
}

impl<T> Default for Members<T> {
    fn default() -> Self {
        Self {
            members: Vec::new(),
        }
    }
}
