#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    name: String,
}

impl Layer {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    layers: Vec<Layer>,
    selected_layer: Option<usize>,
}

impl Document {
    pub fn new() -> Self {
        Self {
            layers: vec![Layer::new("Layer 1")],
            selected_layer: Some(0),
        }
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub fn selected_layer(&self) -> Option<usize> {
        self.selected_layer
    }

    pub fn select_layer(&mut self, index: usize) {
        if index < self.layers.len() {
            self.selected_layer = Some(index);
        }
    }
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_has_a_selected_layer() {
        let document = Document::new();

        assert_eq!(document.layers(), &[Layer::new("Layer 1")]);
        assert_eq!(document.selected_layer(), Some(0));
    }

    #[test]
    fn invalid_layer_selection_is_ignored() {
        let mut document = Document::new();

        document.select_layer(1);

        assert_eq!(document.selected_layer(), Some(0));
    }
}
