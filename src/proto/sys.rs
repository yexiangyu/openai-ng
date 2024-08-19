use smart_default::SmartDefault;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct ModelListResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, SmartDefault)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}
