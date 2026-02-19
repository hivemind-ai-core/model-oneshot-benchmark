//! Core task and artifact models.

pub mod artifact;
pub mod config;
pub mod dependency;
pub mod repository;
pub mod task;

pub use artifact::Artifact;
pub use repository::TaskRepository;
pub use task::{Task, TaskStatus};
