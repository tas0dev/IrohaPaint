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
