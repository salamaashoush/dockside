use gpui::{App, AppContext, Entity, Global};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

static TASK_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_task_id() -> u64 {
  TASK_ID_COUNTER.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
  Running,
  Completed,
  Failed(String),
}

/// A single stage/step within a task
#[derive(Debug, Clone)]
pub struct TaskStage {
  pub description: String,
}

impl TaskStage {
  pub fn new(description: impl Into<String>) -> Self {
    Self {
      description: description.into(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct Task {
  pub id: u64,
  pub description: String,
  pub status: TaskStatus,
  pub progress: Option<f32>, // 0.0 - 1.0
  /// All stages for this task
  pub stages: Vec<TaskStage>,
  /// Current stage index (0-based)
  pub current_stage: usize,
  /// Current stage status message
  pub stage_status: Option<String>,
}

impl Task {
  pub fn new(description: impl Into<String>) -> Self {
    Self {
      id: next_task_id(),
      description: description.into(),
      status: TaskStatus::Running,
      progress: None,
      stages: Vec::new(),
      current_stage: 0,
      stage_status: None,
    }
  }

  pub fn with_stages(mut self, stages: Vec<TaskStage>) -> Self {
    self.stages = stages;
    self
  }

  pub fn is_running(&self) -> bool {
    matches!(self.status, TaskStatus::Running)
  }

  /// Get the current stage if stages are defined
  pub fn current_stage_info(&self) -> Option<&TaskStage> {
    self.stages.get(self.current_stage)
  }

  /// Get display text for the current state
  pub fn display_status(&self) -> String {
    if let Some(status) = &self.stage_status {
      status.clone()
    } else if let Some(stage) = self.current_stage_info() {
      stage.description.clone()
    } else {
      self.description.clone()
    }
  }

  /// Get progress as a fraction based on stages (always 0.0 to 1.0)
  #[allow(clippy::cast_precision_loss)]
  pub fn stage_progress(&self) -> f32 {
    let progress = if self.stages.is_empty() {
      self.progress.unwrap_or(0.0)
    } else if self.stages.len() <= 1 {
      0.0
    } else {
      (self.current_stage as f32) / ((self.stages.len() - 1) as f32)
    };
    // Clamp to valid range for gpui::relative()
    progress.clamp(0.0, 1.0)
  }
}

#[derive(Default)]
pub struct TaskManager {
  tasks: HashMap<u64, Task>,
}

impl TaskManager {
  pub fn new() -> Self {
    Self::default()
  }

  /// Start a new task and return its ID
  pub fn start_task(&mut self, description: impl Into<String>) -> u64 {
    let task = Task::new(description);
    let id = task.id;
    self.tasks.insert(id, task);
    id
  }

  /// Start a new task with predefined stages
  pub fn start_staged_task(&mut self, description: impl Into<String>, stages: Vec<TaskStage>) -> u64 {
    let task = Task::new(description).with_stages(stages);
    let id = task.id;
    self.tasks.insert(id, task);
    id
  }

  /// Advance to the next stage
  pub fn advance_stage(&mut self, task_id: u64) {
    if let Some(task) = self.tasks.get_mut(&task_id)
      && task.current_stage < task.stages.len().saturating_sub(1)
    {
      task.current_stage += 1;
      task.stage_status = None;
    }
  }

  /// Set the running task's progress fraction + a free-form status line.
  pub fn set_progress(&mut self, task_id: u64, progress: f32, status: Option<String>) {
    if let Some(task) = self.tasks.get_mut(&task_id) {
      task.progress = Some(progress.clamp(0.0, 1.0));
      task.stage_status = status;
    }
  }

  /// Mark task as completed
  pub fn complete_task(&mut self, task_id: u64) {
    if let Some(task) = self.tasks.get_mut(&task_id) {
      task.status = TaskStatus::Completed;
      task.progress = Some(1.0);
      task.current_stage = task.stages.len().saturating_sub(1);
    }
    // Remove completed tasks after marking
    self.tasks.remove(&task_id);
  }

  /// Mark task as failed
  pub fn fail_task(&mut self, task_id: u64, error: impl Into<String>) {
    if let Some(task) = self.tasks.get_mut(&task_id) {
      task.status = TaskStatus::Failed(error.into());
    }
    // Remove failed tasks
    self.tasks.remove(&task_id);
  }

  /// Get all running tasks
  pub fn running_tasks(&self) -> Vec<&Task> {
    self.tasks.values().filter(|t| t.is_running()).collect()
  }
}

/// Global wrapper for `TaskManager`
pub struct GlobalTaskManager(pub Entity<TaskManager>);

impl Global for GlobalTaskManager {}

/// Initialize the global task manager
pub fn init_task_manager(cx: &mut App) {
  let manager = cx.new(|_cx| TaskManager::new());
  cx.set_global(GlobalTaskManager(manager));
}

/// Get the global task manager entity
pub fn task_manager(cx: &App) -> Entity<TaskManager> {
  cx.global::<GlobalTaskManager>().0.clone()
}

/// Helper to start a task from any context
pub fn start_task(cx: &mut App, description: impl Into<String>) -> u64 {
  let manager = task_manager(cx);
  manager.update(cx, |m, cx| {
    let id = m.start_task(description);
    cx.notify();
    id
  })
}

/// Helper to start a staged task from any context
pub fn start_staged_task(cx: &mut App, description: impl Into<String>, stages: Vec<TaskStage>) -> u64 {
  let manager = task_manager(cx);
  manager.update(cx, |m, cx| {
    let id = m.start_staged_task(description, stages);
    cx.notify();
    id
  })
}

/// Helper to advance task to next stage
pub fn advance_stage(cx: &mut App, task_id: u64) {
  let manager = task_manager(cx);
  manager.update(cx, |m, cx| {
    m.advance_stage(task_id);
    cx.notify();
  });
}

/// Update progress + status for a task from any context.
pub fn set_task_progress(cx: &mut App, task_id: u64, progress: f32, status: Option<String>) {
  let manager = task_manager(cx);
  manager.update(cx, |m, cx| {
    m.set_progress(task_id, progress, status);
    cx.notify();
  });
}

/// Helper to complete a task from any context
pub fn complete_task(cx: &mut App, task_id: u64) {
  let manager = task_manager(cx);
  manager.update(cx, |m, cx| {
    m.complete_task(task_id);
    cx.notify();
  });
}

/// Helper to fail a task from any context
pub fn fail_task(cx: &mut App, task_id: u64, error: impl Into<String>) {
  let manager = task_manager(cx);
  manager.update(cx, |m, cx| {
    m.fail_task(task_id, error);
    cx.notify();
  });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_task_creation() {
    let task = Task::new("Test task");
    assert_eq!(task.description, "Test task");
    assert!(matches!(task.status, TaskStatus::Running));
    assert!(task.is_running());
    assert!(task.stages.is_empty());
    assert_eq!(task.current_stage, 0);
  }

  #[test]
  fn test_task_with_stages() {
    let stages = vec![
      TaskStage::new("Stage 1"),
      TaskStage::new("Stage 2"),
      TaskStage::new("Stage 3"),
    ];
    let task = Task::new("Staged task").with_stages(stages);

    assert_eq!(task.stages.len(), 3);
    assert_eq!(task.current_stage_info().unwrap().description, "Stage 1");
  }

  #[test]
  fn test_task_display_status() {
    // Simple task - returns description
    let simple_task = Task::new("Simple task");
    assert_eq!(simple_task.display_status(), "Simple task");

    // Staged task - returns current stage description
    let stages = vec![TaskStage::new("First stage"), TaskStage::new("Second stage")];
    let staged_task = Task::new("Staged task").with_stages(stages);
    assert_eq!(staged_task.display_status(), "First stage");
  }

  #[test]
  fn test_task_stage_progress() {
    // No stages - returns 0.0
    let no_stages = Task::new("No stages");
    assert!((no_stages.stage_progress() - 0.0).abs() < 0.01);

    // Single stage - returns 0.0
    let single_stage = Task::new("Single").with_stages(vec![TaskStage::new("Only one")]);
    assert!((single_stage.stage_progress() - 0.0).abs() < 0.01);

    // Multiple stages - calculate progress
    let stages = vec![
      TaskStage::new("Stage 1"),
      TaskStage::new("Stage 2"),
      TaskStage::new("Stage 3"),
      TaskStage::new("Stage 4"),
    ];
    let mut task = Task::new("Multi").with_stages(stages);

    assert!((task.stage_progress() - 0.0).abs() < 0.01); // 0/3
    task.current_stage = 1;
    assert!((task.stage_progress() - 0.333).abs() < 0.01); // 1/3
    task.current_stage = 2;
    assert!((task.stage_progress() - 0.666).abs() < 0.01); // 2/3
    task.current_stage = 3;
    assert!((task.stage_progress() - 1.0).abs() < 0.01); // 3/3
  }

  #[test]
  fn test_task_manager_start_task() {
    let mut manager = TaskManager::new();

    let task_id = manager.start_task("Test task");
    let tasks = manager.running_tasks();

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, task_id);
    assert_eq!(tasks[0].description, "Test task");
  }

  #[test]
  fn test_task_manager_complete_task() {
    let mut manager = TaskManager::new();

    let task_id = manager.start_task("Completing task");
    assert_eq!(manager.running_tasks().len(), 1);

    manager.complete_task(task_id);
    assert!(manager.running_tasks().is_empty());
  }

  #[test]
  fn test_task_manager_fail_task() {
    let mut manager = TaskManager::new();

    let task_id = manager.start_task("Failing task");
    assert_eq!(manager.running_tasks().len(), 1);

    manager.fail_task(task_id, "Something went wrong");
    assert!(manager.running_tasks().is_empty());
  }

  #[test]
  fn test_task_manager_staged_task() {
    let mut manager = TaskManager::new();

    let stages = vec![
      TaskStage::new("Preparing..."),
      TaskStage::new("Downloading..."),
      TaskStage::new("Installing..."),
      TaskStage::new("Verifying..."),
    ];

    let task_id = manager.start_staged_task("Installing package", stages);

    // Initial state
    let tasks = manager.running_tasks();
    assert_eq!(tasks[0].stages.len(), 4);
    assert_eq!(tasks[0].current_stage, 0);

    // Advance stages
    manager.advance_stage(task_id);
    let tasks = manager.running_tasks();
    assert_eq!(tasks[0].current_stage, 1);

    manager.advance_stage(task_id);
    manager.advance_stage(task_id);
    let tasks = manager.running_tasks();
    assert_eq!(tasks[0].current_stage, 3);

    // Should not go past last stage
    manager.advance_stage(task_id);
    manager.advance_stage(task_id);
    let tasks = manager.running_tasks();
    assert_eq!(tasks[0].current_stage, 3);
  }

  #[test]
  fn test_task_manager_multiple_tasks() {
    let mut manager = TaskManager::new();

    let task1 = manager.start_task("Task 1");
    let task2 = manager.start_task("Task 2");
    let task3 = manager.start_task("Task 3");

    assert_eq!(manager.running_tasks().len(), 3);

    manager.complete_task(task2);
    assert_eq!(manager.running_tasks().len(), 2);

    manager.fail_task(task1, "Error");
    assert_eq!(manager.running_tasks().len(), 1);
    assert_eq!(manager.running_tasks()[0].id, task3);
  }

  #[test]
  fn test_task_stage_new() {
    let stage = TaskStage::new("Test stage");
    assert_eq!(stage.description, "Test stage");

    // Test with String
    let stage2 = TaskStage::new(String::from("String stage"));
    assert_eq!(stage2.description, "String stage");
  }

  // Edge case tests for robustness

  #[test]
  fn test_complete_nonexistent_task() {
    let mut manager = TaskManager::new();
    // Should not panic when completing non-existent task
    manager.complete_task(99999);
    assert!(manager.running_tasks().is_empty());
  }

  #[test]
  fn test_fail_nonexistent_task() {
    let mut manager = TaskManager::new();
    // Should not panic when failing non-existent task
    manager.fail_task(99999, "Error");
    assert!(manager.running_tasks().is_empty());
  }

  #[test]
  fn test_advance_stage_on_task_without_stages() {
    let mut manager = TaskManager::new();
    let task_id = manager.start_task("No stages task");

    // Should not panic when advancing stage on task without stages
    manager.advance_stage(task_id);
    let tasks = manager.running_tasks();
    assert_eq!(tasks[0].current_stage, 0);
  }

  #[test]
  fn test_stage_progress_clamping() {
    // Verify progress is always between 0.0 and 1.0
    let mut task = Task::new("Test").with_stages(vec![TaskStage::new("Stage 1"), TaskStage::new("Stage 2")]);

    // Even with invalid current_stage, progress should be clamped
    task.current_stage = 100; // Way beyond stages
    let progress = task.stage_progress();
    assert!((0.0..=1.0).contains(&progress));
  }

  #[test]
  fn test_task_display_status_priority() {
    // stage_status takes priority over stage description
    let mut task = Task::new("Main task").with_stages(vec![TaskStage::new("Stage 1")]);

    // Initially shows stage description
    assert_eq!(task.display_status(), "Stage 1");

    // After setting stage_status, shows that instead
    task.stage_status = Some("Custom status".to_string());
    assert_eq!(task.display_status(), "Custom status");
  }

  #[test]
  fn test_task_ids_are_unique() {
    let task1 = Task::new("Task 1");
    let task2 = Task::new("Task 2");
    let task3 = Task::new("Task 3");

    assert_ne!(task1.id, task2.id);
    assert_ne!(task2.id, task3.id);
    assert_ne!(task1.id, task3.id);
  }
}
