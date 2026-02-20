use crate::jira::types::{Issue, IssueSummary, IssueTypeInfo};
use crate::jira::JiraClient;
use crate::query::Query;
use crate::ui::components::{
  FieldPicker, FieldPickerEvent, InputResult, KeyResult, PickerOption, TextInput,
};
use crate::ui::view::{ShortcutInfo, View, ViewAction};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};

const FIELD_COUNT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
  Title = 0,
  Type = 1,
  Status = 2,
  Epic = 3,
  Labels = 4,
  Description = 5,
}

impl Field {
  fn from_index(i: usize) -> Self {
    match i {
      0 => Field::Title,
      1 => Field::Type,
      2 => Field::Status,
      3 => Field::Epic,
      4 => Field::Labels,
      5 => Field::Description,
      _ => Field::Title,
    }
  }

  fn is_text_input(self) -> bool {
    matches!(self, Field::Title | Field::Labels)
  }
}

enum EditorMode {
  Create {
    project: String,
  },
  Edit {
    key: String,
    original_status_id: String,
  },
}

struct ProjectMetadata {
  issue_types: Vec<IssueTypeInfo>,
  epics: Vec<IssueSummary>,
}

pub struct IssueEditorView {
  mode: EditorMode,
  jira: JiraClient,

  // Form fields
  focused: usize,
  title: TextInput,
  issue_type: FieldPicker,
  status: FieldPicker,
  epic: FieldPicker,
  labels: TextInput,
  description: Option<String>,

  // Data
  metadata_query: Query<ProjectMetadata>,
  issue_query: Option<Query<Issue>>,
  metadata_loaded: bool,
  issue_loaded: bool,

  // Submit
  submit_query: Option<Query<String>>,
  error_message: Option<String>,
  submitting: bool,
  completed: bool,
}

impl IssueEditorView {
  pub fn new_create(
    project: String,
    epic: Option<String>,
    default_labels: Vec<String>,
    jira: JiraClient,
  ) -> Self {
    let jira_meta = jira.clone();
    let project_meta = project.clone();

    let metadata_query = Query::new(move || {
      let jira = jira_meta.clone();
      let project = project_meta.clone();
      async move {
        let (types_result, epics_result) = tokio::join!(
          jira.get_project_statuses(&project),
          jira.get_epics(&project),
        );
        Ok(ProjectMetadata {
          issue_types: types_result.map_err(|e| e.to_string())?,
          epics: epics_result.map_err(|e| e.to_string())?,
        })
      }
    });

    let mut view = Self::base(jira, metadata_query);
    view.mode = EditorMode::Create { project };

    if !default_labels.is_empty() {
      view.labels.set_value(&default_labels.join(", "));
    }
    if let Some(epic_key) = &epic {
      view.epic.set_value(epic_key, epic_key);
    }

    view
  }

  pub fn new_edit(issue: IssueSummary, jira: JiraClient) -> Self {
    let project = issue.key.split('-').next().unwrap_or("").to_string();

    let jira_meta = jira.clone();
    let project_meta = project.clone();
    let metadata_query = Query::new(move || {
      let jira = jira_meta.clone();
      let project = project_meta.clone();
      async move {
        let (types_result, epics_result) = tokio::join!(
          jira.get_project_statuses(&project),
          jira.get_epics(&project),
        );
        Ok(ProjectMetadata {
          issue_types: types_result.map_err(|e| e.to_string())?,
          epics: epics_result.map_err(|e| e.to_string())?,
        })
      }
    });

    let issue_key = issue.key.clone();
    let jira_issue = jira.clone();
    let issue_query = Query::new(move || {
      let jira = jira_issue.clone();
      let key = issue_key.clone();
      async move { jira.get_issue(&key).await.map_err(|e| e.to_string()) }
    });

    let mut view = Self::base(jira, metadata_query);
    view.mode = EditorMode::Edit {
      key: issue.key.clone(),
      original_status_id: issue.status_id.clone(),
    };

    // Pre-fill from summary
    view.title.set_value(&issue.summary);
    view
      .issue_type
      .set_value(&issue.issue_type, &issue.issue_type);
    view.status.set_value(&issue.status_id, &issue.status);
    if let Some(epic) = &issue.epic {
      view.epic.set_value(epic, epic);
    }

    view.issue_query = Some(issue_query);
    view
  }

  fn base(jira: JiraClient, mut metadata_query: Query<ProjectMetadata>) -> Self {
    metadata_query.fetch();
    Self {
      mode: EditorMode::Create {
        project: String::new(),
      },
      jira,
      focused: 0,
      title: TextInput::new(),
      issue_type: FieldPicker::new("Issue Type"),
      status: FieldPicker::new("Status"),
      epic: FieldPicker::new("Epic").with_allow_none(),
      labels: TextInput::new(),
      description: None,
      metadata_query,
      issue_query: None,
      metadata_loaded: false,
      issue_loaded: false,
      submit_query: None,
      error_message: None,
      submitting: false,
      completed: false,
    }
  }

  fn is_create(&self) -> bool {
    matches!(self.mode, EditorMode::Create { .. })
  }

  fn focused_field(&self) -> Field {
    Field::from_index(self.focused)
  }

  fn move_focus(&mut self, delta: i32) {
    self.focused = (self.focused as i32 + delta).rem_euclid(FIELD_COUNT as i32) as usize;
  }

  fn handle_picker_overlays(&mut self, key: KeyEvent) -> Option<ViewAction> {
    match self.issue_type.handle_key(key) {
      KeyResult::Handled => return Some(ViewAction::None),
      KeyResult::Event(FieldPickerEvent::Selected { .. }) => {
        self.update_status_options();
        return Some(ViewAction::None);
      }
      KeyResult::Event(FieldPickerEvent::Cancelled) => return Some(ViewAction::None),
      KeyResult::NotHandled => {}
    }

    match self.status.handle_key(key) {
      KeyResult::Handled => return Some(ViewAction::None),
      KeyResult::Event(_) => return Some(ViewAction::None),
      KeyResult::NotHandled => {}
    }

    match self.epic.handle_key(key) {
      KeyResult::Handled => return Some(ViewAction::None),
      KeyResult::Event(_) => return Some(ViewAction::None),
      KeyResult::NotHandled => {}
    }

    None
  }

  fn handle_global_keys(&mut self, key: KeyEvent) -> Option<ViewAction> {
    if key.code == KeyCode::Char('s') && key.modifiers.contains(KeyModifiers::CONTROL) {
      self.submit();
      return Some(ViewAction::None);
    }
    if key.code == KeyCode::Esc {
      return Some(ViewAction::Pop);
    }
    None
  }

  fn handle_text_input(&mut self, key: KeyEvent) -> Option<ViewAction> {
    let input = match self.focused_field() {
      Field::Title => &mut self.title,
      Field::Labels => &mut self.labels,
      _ => return None,
    };

    match input.handle_key(key) {
      InputResult::Consumed => Some(ViewAction::None),
      InputResult::Submitted(_) => {
        self.move_focus(1);
        Some(ViewAction::None)
      }
      InputResult::Cancelled => None, // Let Esc fall through to global handler
      InputResult::NotHandled => None,
    }
  }

  fn handle_field_navigation(&mut self, key: KeyEvent) -> Option<ViewAction> {
    let in_text = self.focused_field().is_text_input();
    match key.code {
      KeyCode::Tab => {
        self.move_focus(1);
        Some(ViewAction::None)
      }
      KeyCode::BackTab => {
        self.move_focus(-1);
        Some(ViewAction::None)
      }
      KeyCode::Char('j') | KeyCode::Down if !in_text => {
        self.move_focus(1);
        Some(ViewAction::None)
      }
      KeyCode::Char('k') | KeyCode::Up if !in_text => {
        self.move_focus(-1);
        Some(ViewAction::None)
      }
      _ => None,
    }
  }

  fn handle_field_activation(&mut self, key: KeyEvent) -> Option<ViewAction> {
    if key.code != KeyCode::Enter {
      return None;
    }
    match self.focused_field() {
      Field::Type => {
        self.issue_type.show();
        Some(ViewAction::None)
      }
      Field::Status => {
        self.status.show();
        Some(ViewAction::None)
      }
      Field::Epic => {
        self.epic.show();
        Some(ViewAction::None)
      }
      Field::Description => {
        self.launch_editor();
        Some(ViewAction::Redraw)
      }
      _ => None,
    }
  }

  fn update_status_options(&mut self) {
    let Some(meta) = self.metadata_query.data() else {
      return;
    };
    let type_name = self.issue_type.current_label().to_string();
    let Some(issue_type) = meta.issue_types.iter().find(|t| t.name == type_name) else {
      return;
    };

    let options: Vec<PickerOption> = issue_type
      .statuses
      .iter()
      .map(|s| PickerOption {
        id: s.id.clone(),
        label: s.name.clone(),
      })
      .collect();

    let current_valid = options.iter().any(|o| o.id == self.status.current_id());
    self.status.set_options(options);

    if !current_valid {
      if let Some(first) = issue_type.statuses.first() {
        self.status.set_value(&first.id, &first.name);
      }
    }
  }

  fn populate_from_metadata(&mut self) {
    let Some(meta) = self.metadata_query.data() else {
      return;
    };

    let type_options: Vec<PickerOption> = meta
      .issue_types
      .iter()
      .map(|t| PickerOption {
        id: t.name.clone(),
        label: t.name.clone(),
      })
      .collect();

    let first_type = meta.issue_types.first().map(|t| t.name.clone());

    let epic_options: Vec<PickerOption> = meta
      .epics
      .iter()
      .map(|e| PickerOption {
        id: e.key.clone(),
        label: format!("{} - {}", e.key, e.summary),
      })
      .collect();

    // Now we're done borrowing meta, so we can mutate self freely
    self.issue_type.set_options(type_options);
    if self.issue_type.current_id().is_empty() {
      if let Some(name) = &first_type {
        self.issue_type.set_value(name, name);
      }
    }
    self.epic.set_options(epic_options);
    self.update_status_options();

    if self.status.current_id().is_empty() {
      let type_name = self.issue_type.current_label().to_string();
      if let Some(meta) = self.metadata_query.data() {
        if let Some(issue_type) = meta.issue_types.iter().find(|t| t.name == type_name) {
          if let Some(first) = issue_type.statuses.first() {
            let id = first.id.clone();
            let name = first.name.clone();
            self.status.set_value(&id, &name);
          }
        }
      }
    }
  }

  fn populate_from_issue(&mut self) {
    let Some(query) = &self.issue_query else {
      return;
    };
    let Some(issue) = query.data() else { return };
    self.description = issue.description.clone();
    self.labels.set_value(&issue.labels.join(", "));
  }

  fn launch_editor(&mut self) {
    let editor = std::env::var("EDITOR")
      .or_else(|_| std::env::var("VISUAL"))
      .unwrap_or_else(|_| "vi".to_string());

    let tmp_path = std::env::temp_dir().join("j9s-description.md");
    let _ = std::fs::write(&tmp_path, self.description.as_deref().unwrap_or(""));

    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);

    let status = std::process::Command::new(&editor).arg(&tmp_path).status();

    let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::EnterAlternateScreen);
    let _ = crossterm::terminal::enable_raw_mode();

    if status.is_ok() {
      if let Ok(content) = std::fs::read_to_string(&tmp_path) {
        let trimmed = content.trim().to_string();
        self.description = if trimmed.is_empty() {
          None
        } else {
          Some(trimmed)
        };
      }
    }

    let _ = std::fs::remove_file(&tmp_path);
  }

  fn parse_labels(&self) -> Vec<String> {
    self
      .labels
      .value()
      .split(',')
      .map(|s| s.trim().to_string())
      .filter(|s| !s.is_empty())
      .collect()
  }

  fn submit(&mut self) {
    if self.title.value().trim().is_empty() {
      self.error_message = Some("Title is required".to_string());
      return;
    }
    if self.submitting {
      return;
    }

    self.error_message = None;
    self.submitting = true;

    let jira = self.jira.clone();
    let summary = self.title.value().to_string();
    let issue_type = self.issue_type.current_label().to_string();
    let description = self.description.clone();
    let labels = self.parse_labels();
    let epic = if self.epic.current_id().is_empty() {
      None
    } else {
      Some(self.epic.current_id().to_string())
    };
    let target_status_id = self.status.current_id().to_string();

    match &self.mode {
      EditorMode::Create { project, .. } => {
        let project = project.clone();
        let mut query = Query::new(move || {
          let jira = jira.clone();
          let project = project.clone();
          let summary = summary.clone();
          let issue_type = issue_type.clone();
          let description = description.clone();
          let labels = labels.clone();
          let epic = epic.clone();
          let target_status_id = target_status_id.clone();
          async move {
            let key = jira
              .create_issue(
                &project,
                &summary,
                &issue_type,
                description.as_deref(),
                &labels,
                epic.as_deref(),
              )
              .await
              .map_err(|e| e.to_string())?;

            // Fast-track status transition (best-effort)
            if !target_status_id.is_empty() {
              let _ = jira.update_issue_status(&key, &target_status_id).await;
            }

            Ok(key)
          }
        });
        query.fetch();
        self.submit_query = Some(query);
      }
      EditorMode::Edit {
        key,
        original_status_id,
      } => {
        let key = key.clone();
        let original_status_id = original_status_id.clone();
        let mut query = Query::new(move || {
          let jira = jira.clone();
          let key = key.clone();
          let summary = summary.clone();
          let issue_type = issue_type.clone();
          let description = description.clone();
          let labels = labels.clone();
          let epic = epic.clone();
          let target_status_id = target_status_id.clone();
          let original_status_id = original_status_id.clone();
          async move {
            jira
              .update_issue(
                &key,
                &summary,
                description.as_deref(),
                &issue_type,
                &labels,
                epic.as_deref(),
              )
              .await
              .map_err(|e| e.to_string())?;

            if target_status_id != original_status_id && !target_status_id.is_empty() {
              jira
                .update_issue_status(&key, &target_status_id)
                .await
                .map_err(|e| e.to_string())?;
            }

            Ok(key)
          }
        });
        query.fetch();
        self.submit_query = Some(query);
      }
    }
  }

  fn render_form(&self, frame: &mut Frame, area: Rect) {
    let title_str: String;
    let block_title = if self.is_create() {
      " Create Issue "
    } else if let EditorMode::Edit { key, .. } = &self.mode {
      title_str = format!(" Edit {} ", key);
      &title_str
    } else {
      " Edit Issue "
    };

    let block = Block::bordered()
      .title(Line::from(block_title).centered())
      .border_style(Color::Cyan);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 10 || inner.width < 30 {
      return;
    }

    let label_width = 14u16;
    let value_width = inner.width.saturating_sub(label_width + 2);

    let fields: [(&str, Line); FIELD_COUNT] = [
      ("Title:", self.render_title_value()),
      ("Type:", self.render_picker_value(&self.issue_type)),
      ("Status:", self.render_picker_value(&self.status)),
      ("Epic:", self.render_picker_value(&self.epic)),
      ("Labels:", self.render_labels_value()),
      ("Description:", self.render_description_value(value_width)),
    ];

    for (i, (label, value_line)) in fields.iter().enumerate() {
      let y = inner.y + i as u16 * 2;
      if y + 1 >= inner.y + inner.height {
        break;
      }

      let is_focused = i == self.focused;

      let indicator_area = Rect::new(inner.x, y, 1, 1);
      let indicator = if is_focused { ">" } else { " " };
      frame.render_widget(
        Paragraph::new(Span::styled(indicator, Style::new().yellow())),
        indicator_area,
      );

      let label_area = Rect::new(inner.x + 1, y, label_width, 1);
      let label_style = if is_focused {
        Style::new().yellow().bold()
      } else {
        Style::new().dark_gray()
      };
      frame.render_widget(
        Paragraph::new(Span::styled(*label, label_style)),
        label_area,
      );

      let value_area = Rect::new(inner.x + label_width + 1, y, value_width, 1);
      frame.render_widget(Paragraph::new(value_line.clone()), value_area);
    }

    // Status line
    let status_y = inner.y + inner.height.saturating_sub(2);
    let status_area = Rect::new(inner.x + 1, status_y, inner.width - 2, 1);

    if let Some(err) = &self.error_message {
      frame.render_widget(
        Paragraph::new(Span::styled(err.as_str(), Style::new().red())),
        status_area,
      );
    } else if self.submitting {
      frame.render_widget(
        Paragraph::new(Span::styled("Submitting...", Style::new().yellow())),
        status_area,
      );
    } else if !self.metadata_loaded {
      frame.render_widget(
        Paragraph::new(Span::styled("Loading...", Style::new().dark_gray())),
        status_area,
      );
    }

    // Help line
    let help_y = inner.y + inner.height.saturating_sub(1);
    let help_area = Rect::new(inner.x + 1, help_y, inner.width - 2, 1);
    let help = Line::from(vec![
      "Ctrl+S".yellow(),
      " submit  ".dark_gray(),
      "Tab".yellow(),
      " next  ".dark_gray(),
      "Esc".yellow(),
      " cancel".dark_gray(),
    ]);
    frame.render_widget(Paragraph::new(help), help_area);
  }

  fn render_title_value(&self) -> Line<'_> {
    if self.focused_field() == Field::Title {
      let val = self.title.value();
      let (before, after) = val.split_at(self.title.cursor_position());
      Line::from(vec![Span::raw(before), "_".yellow(), Span::raw(after)])
    } else if self.title.is_empty() {
      Line::from("(enter title)".dark_gray())
    } else {
      Line::from(self.title.value().white())
    }
  }

  fn render_picker_value<'a>(&'a self, picker: &'a FieldPicker) -> Line<'a> {
    let label = picker.current_label();
    if label.is_empty() {
      Line::from("(none) \u{25be}".dark_gray())
    } else {
      Line::from(vec![label.cyan(), " \u{25be}".dark_gray()])
    }
  }

  fn render_labels_value(&self) -> Line<'_> {
    if self.focused_field() == Field::Labels {
      let val = self.labels.value();
      let (before, after) = val.split_at(self.labels.cursor_position());
      Line::from(vec![Span::raw(before), "_".yellow(), Span::raw(after)])
    } else if self.labels.is_empty() {
      Line::from("(none)".dark_gray())
    } else {
      Line::from(self.labels.value().white())
    }
  }

  fn render_description_value(&self, width: u16) -> Line<'_> {
    match &self.description {
      None => Line::from("(press Enter to edit)".dark_gray()),
      Some(desc) => {
        let preview = desc.lines().next().unwrap_or("");
        let max = width as usize;
        if preview.len() > max.saturating_sub(3) {
          Line::from(format!("{}...", &preview[..max.saturating_sub(6)]).white())
        } else {
          Line::from(preview.white())
        }
      }
    }
  }
}

impl View for IssueEditorView {
  fn handle_key(&mut self, key: KeyEvent) -> ViewAction {
    if self.completed {
      return ViewAction::Pop;
    }
    if self.submitting {
      return ViewAction::None;
    }

    // Picker overlays first
    if let Some(action) = self.handle_picker_overlays(key) {
      return action;
    }

    // Text input (before global keys, so typing works)
    if self.focused_field().is_text_input() {
      if let Some(action) = self.handle_text_input(key) {
        return action;
      }
    }

    // Global keys (Ctrl+S, Esc)
    if let Some(action) = self.handle_global_keys(key) {
      return action;
    }

    // Field navigation (Tab, j/k, Up/Down)
    if let Some(action) = self.handle_field_navigation(key) {
      return action;
    }

    // Field activation (Enter on picker/description)
    if let Some(action) = self.handle_field_activation(key) {
      return action;
    }

    ViewAction::None
  }

  fn render(&mut self, frame: &mut Frame, area: Rect) {
    self.render_form(frame, area);
    self.issue_type.render_overlay(frame, area);
    self.status.render_overlay(frame, area);
    self.epic.render_overlay(frame, area);
  }

  fn breadcrumb_label(&self) -> String {
    if self.is_create() {
      "Create".to_string()
    } else if let EditorMode::Edit { key, .. } = &self.mode {
      format!("Edit {}", key)
    } else {
      "Edit".to_string()
    }
  }

  fn tick(&mut self) {
    if !self.metadata_loaded {
      self.metadata_query.poll();
      if self.metadata_query.data().is_some() {
        self.metadata_loaded = true;
        self.populate_from_metadata();
      }
      if let Some(err) = self.metadata_query.error() {
        self.error_message = Some(format!("Failed to load metadata: {}", err));
      }
    }

    if !self.issue_loaded {
      let (loaded, err) = match &mut self.issue_query {
        Some(query) => {
          query.poll();
          let loaded = query.data().is_some();
          let err = query
            .error()
            .map(|e| format!("Failed to load issue: {}", e));
          (loaded, err)
        }
        None => (true, None),
      };
      if let Some(err) = err {
        self.error_message = Some(err);
      }
      if loaded {
        self.issue_loaded = true;
        self.populate_from_issue();
      }
    }

    if let Some(query) = &mut self.submit_query {
      query.poll();
      if query.data().is_some() {
        self.completed = true;
        self.submitting = false;
      }
      if let Some(err) = query.error() {
        self.error_message = Some(format!("Submit failed: {}", err));
        self.submitting = false;
        self.submit_query = None;
      }
    }
  }

  fn shortcuts(&self) -> Vec<ShortcutInfo> {
    vec![
      ShortcutInfo::new("Ctrl+S", "submit").with_priority(10),
      ShortcutInfo::new("Tab", "next field").with_priority(20),
      ShortcutInfo::new("Esc", "cancel").with_priority(30),
    ]
  }
}
