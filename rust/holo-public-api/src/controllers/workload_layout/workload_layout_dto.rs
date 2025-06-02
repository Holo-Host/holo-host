use db_utils::schemas;

#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, utoipa::ToSchema)]
pub struct CreateWorkloadLayoutDto {}

pub fn from_workload_layout_dto(
    _dto: CreateWorkloadLayoutDto,
) -> schemas::workload_layout::WorkloadLayout {
    schemas::workload_layout::WorkloadLayout {
        ..Default::default()
    }
}
