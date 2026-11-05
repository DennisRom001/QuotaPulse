use crate::providers::ProviderDescriptor;
use crate::usage::models::ProviderId;

pub fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: ProviderId::Antigravity,
        display_name: "Gemini",
        brand_color: "#4f6bed",
    }
}
