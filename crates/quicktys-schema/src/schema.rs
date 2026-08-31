//! The self-describing schema model.
//!
//! A [`Schema`] is the typed, self-contained description of one field. It drives
//! three things (ARCHITECTURE.md §5):
//!   1. UI form generation in the WebView,
//!   2. validation of JSON payloads crossing the WASM ABI ([`crate::validate`]),
//!   3. YAML/JSON (de)serialization of persisted values.
//!
//! The model covers the base types (String, Number, Boolean, Object, Array) plus
//! the rich UI types that make CastMate's forms expressive (Duration, Color,
//! FilePath, Range, Toggle, Resource). Adding a new rich type means adding a
//! [`SchemaType`] variant and a match arm in [`crate::validate`] — the ABI does
//! not change, because payloads stay JSON validated against the schema.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A field schema: its type (and type-specific options) plus shared metadata.
///
/// Serializes with an internally-tagged `type` discriminator, e.g.
/// `{ type: "Number", min: 0, max: 100 }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Schema {
    String {
        #[serde(flatten, default)]
        common: Common,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
        /// Closed set of allowed values (CastMate's `enum`).
        #[serde(default, rename = "enum", skip_serializing_if = "Option::is_none")]
        enum_values: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_length: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_length: Option<usize>,
        #[serde(default, skip_serializing_if = "is_false")]
        multiline: bool,
        /// Field may contain `{{ }}` expressions (deferred type check, see
        /// [`crate::validate`]).
        #[serde(default, skip_serializing_if = "is_false")]
        template: bool,
    },
    Number {
        #[serde(flatten, default)]
        common: Common,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        step: Option<f64>,
        #[serde(default, skip_serializing_if = "is_false")]
        slider: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        template: bool,
    },
    Boolean {
        #[serde(flatten, default)]
        common: Common,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<bool>,
    },
    Object {
        #[serde(flatten, default)]
        common: Common,
        /// Ordered fields (order is meaningful for the generated form).
        #[serde(default)]
        properties: IndexMap<String, Schema>,
    },
    Array {
        #[serde(flatten, default)]
        common: Common,
        items: Box<Schema>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_items: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max_items: Option<usize>,
    },
    /// A length of time, in **seconds** (CastMate's `Duration`).
    Duration {
        #[serde(flatten, default)]
        common: Common,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        #[serde(default, skip_serializing_if = "is_false")]
        template: bool,
    },
    /// An RGBA/hex color string.
    Color {
        #[serde(flatten, default)]
        common: Common,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        template: bool,
    },
    /// A filesystem path (file or media). Stored as a string.
    FilePath {
        #[serde(flatten, default)]
        common: Common,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
        #[serde(default, skip_serializing_if = "is_false")]
        template: bool,
    },
    /// A numeric range `{ min, max }`; both bounds optional and templatable.
    Range {
        #[serde(flatten, default)]
        common: Common,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<RangeValue>,
        #[serde(default, skip_serializing_if = "is_false")]
        template: bool,
    },
    /// A tri-state toggle: `true`, `false`, or `"toggle"` (flip current state).
    Toggle {
        #[serde(flatten, default)]
        common: Common,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<serde_json::Value>,
    },
    /// A reference to a host resource of `resource_type`, by id (a string).
    Resource {
        #[serde(flatten, default)]
        common: Common,
        #[serde(rename = "resourceType")]
        resource_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<String>,
    },
}

/// Metadata shared by every schema variant.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Common {
    /// Human-facing label key (an i18n key, not a translated string).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Whether the field must be present (checked when this schema is an object
    /// property — see [`crate::validate`]).
    #[serde(default, skip_serializing_if = "is_false")]
    pub required: bool,
    /// UI hint: a named source the editor should offer as autocomplete for this
    /// string field (e.g. `"alert"` → the names of existing alert widgets). Does
    /// not constrain the value; it's a free string with suggestions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggest: Option<String>,
}

/// The value of a [`Schema::Range`] field.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct RangeValue {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl Schema {
    /// Shared metadata for this schema.
    pub fn common(&self) -> &Common {
        match self {
            Schema::String { common, .. }
            | Schema::Number { common, .. }
            | Schema::Boolean { common, .. }
            | Schema::Object { common, .. }
            | Schema::Array { common, .. }
            | Schema::Duration { common, .. }
            | Schema::Color { common, .. }
            | Schema::FilePath { common, .. }
            | Schema::Range { common, .. }
            | Schema::Toggle { common, .. }
            | Schema::Resource { common, .. } => common,
        }
    }

    /// Mutable access to shared metadata (for builders).
    pub fn common_mut(&mut self) -> &mut Common {
        match self {
            Schema::String { common, .. }
            | Schema::Number { common, .. }
            | Schema::Boolean { common, .. }
            | Schema::Object { common, .. }
            | Schema::Array { common, .. }
            | Schema::Duration { common, .. }
            | Schema::Color { common, .. }
            | Schema::FilePath { common, .. }
            | Schema::Range { common, .. }
            | Schema::Toggle { common, .. }
            | Schema::Resource { common, .. } => common,
        }
    }

    /// Set the label key (i18n) for this field.
    pub fn named(mut self, name: &str) -> Schema {
        self.common_mut().name = Some(name.to_string());
        self
    }

    /// Attach an autocomplete-source hint (see [`Common::suggest`]).
    pub fn suggest(mut self, source: &str) -> Schema {
        self.common_mut().suggest = Some(source.to_string());
        self
    }

    /// Constrain a `String` schema to a closed set of values (rendered as a
    /// dropdown). No-op on non-string schemas.
    pub fn options(mut self, values: impl IntoIterator<Item = impl Into<String>>) -> Schema {
        if let Schema::String { enum_values, .. } = &mut self {
            *enum_values = Some(values.into_iter().map(Into::into).collect());
        }
        self
    }

    /// Whether this field opts into `{{ }}` templating. Templatable fields skip
    /// strict type-checking when their value is itself a template string.
    pub fn is_templatable(&self) -> bool {
        matches!(
            self,
            Schema::String { template: true, .. }
                | Schema::Number { template: true, .. }
                | Schema::Duration { template: true, .. }
                | Schema::Color { template: true, .. }
                | Schema::FilePath { template: true, .. }
                | Schema::Range { template: true, .. }
        )
    }

    /// The declared default as a JSON value, if any (used to seed forms/configs).
    pub fn default_value(&self) -> Option<serde_json::Value> {
        use serde_json::to_value;
        match self {
            Schema::String { default, .. } => default.as_ref().map(|d| to_value(d).unwrap()),
            Schema::Number { default, .. } | Schema::Duration { default, .. } => {
                default.map(|d| to_value(d).unwrap())
            }
            Schema::Boolean { default, .. } => default.map(|d| to_value(d).unwrap()),
            Schema::Color { default, .. }
            | Schema::FilePath { default, .. }
            | Schema::Resource { default, .. } => {
                default.as_ref().map(|d| to_value(d).unwrap())
            }
            Schema::Range { default, .. } => default.map(|d| to_value(d).unwrap()),
            Schema::Toggle { default, .. } => default.clone(),
            Schema::Object { .. } | Schema::Array { .. } => None,
        }
    }
}

impl Schema {
    /// Build an object schema from ordered `(field_name, schema)` pairs.
    pub fn object(fields: impl IntoIterator<Item = (String, Schema)>) -> Schema {
        Schema::Object {
            common: Common::default(),
            properties: fields.into_iter().collect(),
        }
    }

    /// A plain unconstrained `Number` schema.
    pub fn number() -> Schema {
        Schema::Number {
            common: Common::default(),
            default: None,
            min: None,
            max: None,
            step: None,
            slider: false,
            unit: None,
            template: false,
        }
    }

    /// A plain unconstrained `String` schema.
    pub fn string() -> Schema {
        Schema::String {
            common: Common::default(),
            default: None,
            enum_values: None,
            min_length: None,
            max_length: None,
            multiline: false,
            template: false,
        }
    }

    /// A `String` schema marked as templatable (`@var` / `{{ }}`), so the editor
    /// shows the highlighted template input.
    pub fn template_string() -> Schema {
        Schema::String {
            common: Common::default(),
            default: None,
            enum_values: None,
            min_length: None,
            max_length: None,
            multiline: false,
            template: true,
        }
    }

    /// A plain `Boolean` schema.
    pub fn boolean() -> Schema {
        Schema::Boolean {
            common: Common::default(),
            default: None,
        }
    }

    /// An `Array` schema whose items match `items`.
    pub fn array(items: Schema) -> Schema {
        Schema::Array {
            common: Common::default(),
            items: Box::new(items),
            min_items: None,
            max_items: None,
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_schema_roundtrips_through_yaml() {
        let yaml = "type: Number\nname: volume\nmin: 0\nmax: 100\nslider: true\n";
        let s: Schema = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(
            s,
            Schema::Number { slider: true, min: Some(0.0), max: Some(100.0), .. }
        ));
        assert_eq!(s.common().name.as_deref(), Some("volume"));
        // Re-serializing drops defaults/false flags but keeps set fields.
        let back = serde_yaml::to_string(&s).unwrap();
        let reparsed: Schema = serde_yaml::from_str(&back).unwrap();
        assert_eq!(s, reparsed);
    }

    #[test]
    fn object_preserves_property_order() {
        let yaml = "\
type: Object
properties:
  gamma: { type: Boolean }
  alpha: { type: String }
  beta:  { type: Number }
";
        let s: Schema = serde_yaml::from_str(yaml).unwrap();
        let Schema::Object { properties, .. } = s else {
            panic!("expected object")
        };
        let keys: Vec<&str> = properties.keys().map(String::as_str).collect();
        assert_eq!(keys, ["gamma", "alpha", "beta"]);
    }

    #[test]
    fn defaults_extract_as_json() {
        let s = Schema::Duration {
            common: Common::default(),
            default: Some(3.0),
            min: None,
            max: None,
            template: false,
        };
        assert_eq!(s.default_value(), Some(serde_json::json!(3.0)));
    }
}
