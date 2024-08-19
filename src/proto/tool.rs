use crate::error::*;
use serde_with::skip_serializing_none;
use smart_default::SmartDefault;
use std::collections::HashMap;

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub typ: Option<String>,
    pub function: Function,
}

impl From<Function> for ToolCall {
    fn from(f: Function) -> Self {
        ToolCall {
            id: None,
            typ: Some("function".to_string()),
            function: f,
        }
    }
}

impl ToolCall {
    pub fn builder() -> ToolCallBuilder {
        ToolCallBuilder::default()
    }
}

#[derive(Debug, Clone, SmartDefault)]
pub struct ToolCallBuilder {
    pub id: Option<String>,
    #[default(Some("function".to_string()))]
    typ: Option<String>,
    function: Option<Function>,
}

impl ToolCallBuilder {
    pub fn with_function(mut self, function: impl Into<Function>) -> Self {
        self.function = Some(function.into());
        self
    }

    pub fn build(self) -> Result<ToolCall> {
        let Self { id, typ, function } = self;
        let typ = typ.ok_or(Error::ToolCallBuild)?;
        let function = function.ok_or(Error::ToolCallBuild)?;
        Ok(ToolCall {
            id,
            typ: Some(typ),
            function,
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Function {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<Parameters>,
    pub arguments: Option<String>,
}

pub mod serde_value {

    use serde::de::{self, Deserialize, DeserializeOwned, Deserializer};
    use serde::ser::{self, Serialize, Serializer};
    use serde_json;

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        let j = serde_json::to_string(value).map_err(ser::Error::custom)?;
        j.serialize(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: DeserializeOwned,
        D: Deserializer<'de>,
    {
        let j = String::deserialize(deserializer)?;
        serde_json::from_str(&j).map_err(de::Error::custom)
    }
}

impl Function {
    pub fn builder() -> FunctionBuilder {
        FunctionBuilder::default()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct FunctionBuilder {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<Parameters>,
    pub arguments: Option<String>,
}

impl FunctionBuilder {
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_parameters(mut self, parameters: Parameters) -> Self {
        self.parameters = Some(parameters);
        self
    }

    pub fn build(self) -> Result<Function> {
        let Self {
            name,
            description,
            parameters,
            arguments,
        } = self;

        let name = name.ok_or(Error::ToolCallFunctionBuild)?;

        Ok(Function {
            name: Some(name),
            description,
            parameters,
            arguments,
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
#[serde(untagged)]
pub enum Argument {
    string(String),
    number(f64),
    integer(i64),
    boolean(bool),
    array(Vec<Argument>),
    object(HashMap<String, Argument>),
}

macro_rules! impl_argument_as_value {
    ($fun: ident, $i: ident, $typ: ty) => {
        impl Argument {
            pub fn $fun(&self) -> Option<&$typ> {
                match self {
                    Self::$i(inner) => Some(&inner),
                    _ => None,
                }
            }
        }
    };
}

impl_argument_as_value!(as_string, string, String);
impl_argument_as_value!(as_number, number, f64);
impl_argument_as_value!(as_integer, integer, i64);
impl_argument_as_value!(as_boolean, boolean, bool);
impl_argument_as_value!(as_array, array, Vec<Argument>);
impl_argument_as_value!(as_object, object, HashMap<String, Argument>);

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Parameters {
    #[serde(rename = "type")]
    pub typ: String,
    pub properties: HashMap<String, ParameterProperty>,
    #[serde(default)]
    pub required: Vec<String>,
}

impl Parameters {
    pub fn builder() -> ParametersBuilder {
        ParametersBuilder::default()
    }
}

#[derive(Debug, Clone, SmartDefault)]
pub struct ParametersBuilder {
    #[default(Some("object".to_string()))]
    typ: Option<String>,
    properties: HashMap<String, ParameterProperty>,
    required: Vec<String>,
}

impl ParametersBuilder {
    pub fn add_property(mut self, name: impl Into<String>, property: ParameterProperty) -> Self {
        self.properties.insert(name.into(), property);
        self
    }

    pub fn add_required(mut self, name: impl Into<String>) -> Self {
        self.required.push(name.into());
        self
    }

    pub fn build(self) -> Result<Parameters> {
        let Self {
            typ,
            properties,
            required,
        } = self;

        let typ = typ.ok_or(Error::ToolCallParametersBuild)?;

        Ok(Parameters {
            typ,
            properties,
            required,
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParameterProperty {
    #[serde(rename = "type")]
    pub typ: Option<ParameterType>,
    pub description: String,
    pub items: Option<HashMap<String, String>>,
}

impl ParameterProperty {
    pub fn builder() -> ParameterPropertyBuilder {
        ParameterPropertyBuilder::default()
    }
}

#[derive(Debug, Clone, SmartDefault)]
pub struct ParameterPropertyBuilder {
    typ: Option<ParameterType>,
    description: Option<String>,
    items: Option<HashMap<String, String>>,
}

impl ParameterPropertyBuilder {
    pub fn with_type(mut self, typ: ParameterType) -> Self {
        self.typ = Some(typ);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_items(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if self.items.is_none() {
            self.items = Some(HashMap::new());
        }

        self.items
            .as_mut()
            .unwrap()
            .insert(key.into(), value.into());
        self
    }

    pub fn build(self) -> Result<ParameterProperty> {
        let Self {
            typ,
            description,
            items,
        } = self;

        let typ = typ.ok_or(Error::ToolCallParametersBuild)?;
        let description = description.ok_or(Error::ToolCallParametersBuild)?;

        Ok(ParameterProperty {
            typ: Some(typ),
            description,
            items,
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[allow(non_camel_case_types)]
pub enum ParameterType {
    string,
    number,
    integer,
    boolean,
    array,
    object,
}
