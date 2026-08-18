use core_engine::PodcastContextDraft;
use language_engine::{DisambiguationRequest, DisambiguationResponse, PreparedDisambiguation};
use serde::{Deserialize, Serialize};

pub type CandidateSenseDto = language_engine::CandidateSense;
pub type DisambiguationRequestDto = DisambiguationRequest;
pub type DisambiguationResponseDto = DisambiguationResponse;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepareDisambiguationInputDto {
    pub draft: PodcastContextDraft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDisambiguationDto {
    pub request: DisambiguationRequestDto,
    pub system_prompt: String,
    pub user_prompt: String,
}

impl From<PreparedDisambiguation> for PreparedDisambiguationDto {
    fn from(value: PreparedDisambiguation) -> Self {
        Self {
            request: value.request,
            system_prompt: value.system_prompt,
            user_prompt: value.user_prompt,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateDisambiguationResponseInputDto {
    pub request: DisambiguationRequestDto,
    pub response_json: String,
}
