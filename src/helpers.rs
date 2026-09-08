use dialoguer::{Input};
use rand::{RngExt};

pub fn generate_numeric_code() -> String {
    rand::rng().random_range(1111..9999).to_string()
}

pub fn get_input(prompt_string: &String) -> String {
    Input::new()
    .with_prompt(prompt_string)
    .interact_text()
    .unwrap()
}