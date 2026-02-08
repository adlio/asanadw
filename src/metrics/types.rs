use serde::Serialize;

/// Throughput metrics: tasks created, completed, and net flow.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ThroughputMetrics {
    pub tasks_created: u64,
    pub tasks_completed: u64,
    pub net_new: i64,
}

/// Health metrics: overdue tasks, unassigned tasks, stale tasks.
#[derive(Debug, Clone, Default, Serialize)]
pub struct HealthMetrics {
    pub overdue_count: u64,
    pub unassigned_count: u64,
    /// Tasks not modified in 14+ days.
    pub stale_count: u64,
    pub total_open: u64,
    /// Percentage of open tasks that are overdue.
    pub overdue_pct: f64,
    /// Percentage of open tasks that are unassigned.
    pub unassigned_pct: f64,
}

/// Lead time metrics: how long tasks take to complete.
#[derive(Debug, Clone, Default, Serialize)]
pub struct LeadTimeMetrics {
    pub avg_days_to_complete: Option<f64>,
    pub median_days_to_complete: Option<f64>,
    pub p90_days_to_complete: Option<f64>,
    pub min_days_to_complete: Option<i32>,
    pub max_days_to_complete: Option<i32>,
}

/// Collaboration metrics: comments, likes, followers.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CollaborationMetrics {
    pub total_comments: u64,
    pub unique_commenters: u64,
    pub total_likes: u64,
}

/// Aggregated metrics for a user over a period.
#[derive(Debug, Clone, Serialize)]
pub struct UserMetrics {
    pub user_gid: String,
    pub user_name: Option<String>,
    pub period_key: String,
    pub throughput: ThroughputMetrics,
    pub lead_time: LeadTimeMetrics,
    pub collaboration: CollaborationMetrics,
}

/// Aggregated metrics for a project over a period.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectMetrics {
    pub project_gid: String,
    pub project_name: Option<String>,
    pub period_key: String,
    pub throughput: ThroughputMetrics,
    pub health: HealthMetrics,
    pub lead_time: LeadTimeMetrics,
    pub collaboration: CollaborationMetrics,
}

/// Aggregated metrics for a portfolio over a period.
#[derive(Debug, Clone, Serialize)]
pub struct PortfolioMetrics {
    pub portfolio_gid: String,
    pub portfolio_name: Option<String>,
    pub period_key: String,
    pub throughput: ThroughputMetrics,
    pub health: HealthMetrics,
    pub lead_time: LeadTimeMetrics,
    pub collaboration: CollaborationMetrics,
    pub project_count: u64,
}

/// Aggregated metrics for a team over a period.
#[derive(Debug, Clone, Serialize)]
pub struct TeamMetrics {
    pub team_gid: String,
    pub team_name: Option<String>,
    pub period_key: String,
    pub throughput: ThroughputMetrics,
    pub health: HealthMetrics,
    pub lead_time: LeadTimeMetrics,
    pub collaboration: CollaborationMetrics,
    pub member_count: u64,
}
