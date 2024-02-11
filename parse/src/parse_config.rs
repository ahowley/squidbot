use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct PlayerConfig {
    pub pronouns: Vec<String>,
    pub deadnames: Vec<String>,
}

#[derive(Deserialize)]
pub struct AliasConfig {
    pub player: String,
    pub senders: Vec<String>,
}

#[derive(Deserialize)]
pub struct CampaignConfig {
    pub log: String,
    pub dungeon_master: String,
    pub timezone_offset: i32,
    pub aliases: Vec<AliasConfig>,
}

#[derive(Deserialize)]
pub struct Config {
    pub players: HashMap<String, PlayerConfig>,
    pub replace_all_deadnames_with: String,
    pub campaigns: HashMap<String, CampaignConfig>,
}

impl Config {
    pub fn parse(config_json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(config_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config() {
        let test_config_raw = r#"
        {
            "players": {
                "Alex": {
                    "pronouns": ["he/him/his/his", "they/them/their/theirs"],
                    "deadnames": [""]
                },
                "Bob": {
                    "pronouns": ["he/him/his/his"],
                    "deadnames": [""]
                },
                "Sally": {
                    "pronouns": ["she/her/her/hers"],
                    "deadnames": [""]
                }
            },
            "replace_all_deadnames_with": ":)",
            "campaigns": {
                "Curse of Strahd": {
                    "log": "r20_curse_of_strahd.html",
                    "dungeon_master": "Bob",
                    "timezone_offset": -6,
                    "aliases": [
                        {
                            "player": "Bob",
                            "senders": ["cool_guy 420"]
                        }
                    ]
                },
                "Descent into Avernus": {
                    "log": "fnd_descent_into_avernus.db",
                    "dungeon_master": "Sally",
                    "timezone_offset": -6,
                    "aliases": [
                        {
                            "player": "Bob",
                            "senders": ["cool_guy 421"]
                        }
                    ]
                }
            }
        }
        "#;
        let test_config = Config::parse(test_config_raw).unwrap();
        assert_eq!(test_config.players.len(), 3);
        assert_eq!(
            test_config.players.get("Alex").unwrap().pronouns[0],
            "he/him/his/his".to_owned()
        );
        assert_eq!(test_config.replace_all_deadnames_with, ":)".to_owned());
        assert_eq!(test_config.campaigns.len(), 2);
        assert_eq!(
            test_config
                .campaigns
                .get("Curse of Strahd")
                .unwrap()
                .aliases[0]
                .senders[0],
            "cool_guy 420".to_owned()
        );
    }
}
