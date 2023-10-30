use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct ModelOptions {
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub max_tokens: Option<u32>,
}

fn merge_option<T: Clone>(a: &mut Option<T>, b: &Option<T>) {
    if a.is_none() && b.is_some() {
        *a = b.clone();
    }
}

impl ModelOptions {
    /// For any members that are `None` in this `ModelOptions`, use the value from `other`
    pub fn merge_defaults(&mut self, other: &ModelOptions) {
        merge_option(&mut self.model, &other.model);
        merge_option(&mut self.temperature, &other.temperature);
        merge_option(&mut self.top_p, &other.top_p);
        merge_option(&mut self.frequency_penalty, &other.frequency_penalty);
        merge_option(&mut self.presence_penalty, &other.presence_penalty);
        merge_option(&mut self.stop, &other.stop);
        merge_option(&mut self.max_tokens, &other.max_tokens);
    }
}
