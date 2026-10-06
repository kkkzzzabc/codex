//! Validation and schema for the optional prose column limit.
use schemars::JsonSchema;
use schemars::r#gen::SchemaGenerator;
use schemars::schema::Schema;
use serde::Deserialize;
use serde::Deserializer;

pub(crate) fn schema(generator: &mut SchemaGenerator) -> Schema {
    let mut schema = Option::<usize>::json_schema(generator).into_object();
    schema.number().minimum = Some(40.0);
    schema.into()
}

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let width = Option::<usize>::deserialize(deserializer)?;
    if width.is_some_and(|width| width < 40) {
        return Err(serde::de::Error::custom(
            "tui.max_prose_width must be an integer of at least 40 columns",
        ));
    }
    Ok(width)
}

#[cfg(test)]
mod tests {
    use crate::types::Tui;
    #[test]
    fn validates_prose_width() {
        for value in ["40", "80", "100"] {
            let tui: Tui = toml::from_str(&format!("max_prose_width = {value}")).unwrap();
            assert_eq!(tui.max_prose_width, Some(value.parse().unwrap()));
        }
        assert_eq!(toml::from_str::<Tui>("").unwrap().max_prose_width, None);
        for value in ["0", "39", "-1", "80.5", "\"80\""] {
            assert!(
                toml::from_str::<Tui>(&format!("max_prose_width = {value}")).is_err(),
                "{value}"
            );
        }
    }
}
