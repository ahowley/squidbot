use serde::Deserialize;

#[derive(Deserialize)]
pub struct RandomMessageTemplates {
    pub super_templates: Vec<String>,
    pub templates: Vec<String>,
    pub words: Vec<String>,
}

impl RandomMessageTemplates {
    pub fn parse(random_message_templates_json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(random_message_templates_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_random_message_templates() {
        let test_random_message_templates_raw = r#"{
            "super_templates": ["%a\nand then %b", "%a\nor %b", "%a\nbut %b", "%a\ntherefore %b"],
            "templates": ["%x ahead", "Let there be %x", "%x", "%x!", "%x?", "%x..."],
            "words": ["item", "close-quarters battle", "attacking", "morning", "high road", "east"]
        }
        "#;
        let test_random_message_templates =
            RandomMessageTemplates::parse(test_random_message_templates_raw).unwrap();

        assert_eq!(test_random_message_templates.super_templates.len(), 4);
        assert_eq!(test_random_message_templates.templates[0], "%x ahead");
    }
}
