//! Create / edit form for one Colima profile. Options, locks and checks all come from
//! `telima_core::rules`; this view only draws them and shows violations next to their field.
use std::sync::{Arc, Mutex};

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use telima_app::AppError;
use telima_app::runtime::App as Telima;
use telima_app::view::daily::{MOUNTS_EMPTY, MOUNTS_HELP, kubernetes_copy, restart_required_text};
use telima_app::view::notice::error_text;
use telima_core::profile::{HostOs, Mount, MountType, ProfileName, ProfileSpec, Runtime, VmType};
use telima_core::rules::{self, RuleViolation};

use super::progress::{ProgressEnd, StartProgress};
use super::widgets::{Tone, button, edit_text, error_line, field_row, hint_line, text_box};
use crate::components::dialog::fade_in;
use crate::components::paragraph::paragraph;
use crate::components::segmented::segmented;
use crate::components::switch::switch;
use crate::style::{hsla, palette, space};
use crate::theme::{font, metric};

/// Colima's own defaults for a new profile.
const NEW_CPUS: u32 = 2;
const NEW_MEMORY_GIB: u32 = 2;
const NEW_DISK_GIB: u32 = 100;

/// Switch ids for the mount rows (the shared switch takes a static id); also the row cap.
const MOUNT_SWITCH_IDS: [&str; 8] = [
    "mount-writable-0",
    "mount-writable-1",
    "mount-writable-2",
    "mount-writable-3",
    "mount-writable-4",
    "mount-writable-5",
    "mount-writable-6",
    "mount-writable-7",
];

/// Form fields. `key` matches the field names used by `rules`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Cpus,
    Memory,
    Disk,
    VmType,
    MountType,
    Runtime,
    Rosetta,
    NetworkAddress,
    Mounts,
    Kubernetes,
}

impl Field {
    pub fn key(self) -> &'static str {
        match self {
            Field::Name => "name",
            Field::Cpus => "cpus",
            Field::Memory => "memory_gib",
            Field::Disk => "disk_gib",
            Field::VmType => "vm_type",
            Field::MountType => "mount_type",
            Field::Runtime => "runtime",
            Field::Rosetta => "rosetta",
            Field::NetworkAddress => "network_address",
            Field::Mounts => "mounts",
            Field::Kubernetes => "kubernetes",
        }
    }

    const ALL: [Field; 11] = [
        Field::Name,
        Field::Cpus,
        Field::Memory,
        Field::Disk,
        Field::VmType,
        Field::MountType,
        Field::Runtime,
        Field::Rosetta,
        Field::NetworkAddress,
        Field::Mounts,
        Field::Kubernetes,
    ];

    fn from_key(key: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| f.key() == key)
    }

    fn of(v: &RuleViolation) -> Field {
        match v {
            RuleViolation::VmTypeUnavailable(_) => Field::VmType,
            RuleViolation::MountTypeUnavailable { .. } => Field::MountType,
            RuleViolation::RosettaUnavailable => Field::Rosetta,
            RuleViolation::RuntimeUnsupported(_) => Field::Runtime,
            RuleViolation::TooSmall { field, .. } | RuleViolation::Immutable(field) => {
                Field::from_key(field).unwrap_or(Field::Name)
            }
            RuleViolation::DiskShrink { .. } => Field::Disk,
            RuleViolation::MountPathNotAbsolute(_) | RuleViolation::DuplicateMount(_) => {
                Field::Mounts
            }
            RuleViolation::KubernetesNeedsDocker(_) => Field::Kubernetes,
        }
    }
}

/// Korean UI copy for a rule violation.
pub fn violation_text(v: &RuleViolation) -> String {
    match v {
        RuleViolation::VmTypeUnavailable(vm) => {
            format!("이 OS에서는 {} VM을 쓸 수 없습니다", vm_label(*vm))
        }
        RuleViolation::MountTypeUnavailable { vm, mount } => format!(
            "{} 마운트는 {} VM에서 쓸 수 없습니다",
            mount_label(*mount),
            vm_label(*vm)
        ),
        RuleViolation::RosettaUnavailable => "Rosetta는 macOS의 vz VM에서만 쓸 수 있습니다".into(),
        RuleViolation::RuntimeUnsupported(_) => "docker 런타임만 지원합니다".into(),
        RuleViolation::TooSmall { min, .. } => format!("{min} 이상이어야 합니다"),
        RuleViolation::Immutable(_) => "만든 뒤에는 바꿀 수 없습니다".into(),
        RuleViolation::DiskShrink { from, to } => {
            format!("디스크는 늘리기만 할 수 있습니다 ({from} GiB → {to} GiB)")
        }
        other => telima_app::view::daily::mount_violation_text(other).unwrap_or_default(),
    }
}

fn vm_label(vm: VmType) -> &'static str {
    match vm {
        VmType::Vz => "vz",
        VmType::Qemu => "qemu",
        VmType::Krunkit => "krunkit",
    }
}

fn mount_label(m: MountType) -> &'static str {
    match m {
        MountType::Virtiofs => "virtiofs",
        MountType::Sshfs => "sshfs",
        MountType::NineP => "9p",
    }
}

/// What the user has entered so far. `name` stays raw text until submit.
#[derive(Debug, Clone, PartialEq)]
pub struct Draft {
    pub name: String,
    pub cpus: u32,
    pub memory_gib: u32,
    pub disk_gib: u32,
    pub vm_type: VmType,
    pub mount_type: MountType,
    pub runtime: Runtime,
    pub rosetta: bool,
    pub network_address: bool,
    pub mounts: Vec<Mount>,
    pub kubernetes: bool,
}

impl Draft {
    fn new_for(os: HostOs) -> Self {
        let vm_type = rules::vm_types(os)[0];
        Self {
            name: String::new(),
            cpus: NEW_CPUS,
            memory_gib: NEW_MEMORY_GIB,
            disk_gib: NEW_DISK_GIB,
            vm_type,
            mount_type: rules::mount_types(vm_type)[0],
            runtime: rules::SUPPORTED_RUNTIMES[0],
            rosetta: false,
            network_address: false,
            mounts: Vec::new(),
            kubernetes: false,
        }
    }

    fn from_spec(s: &ProfileSpec) -> Self {
        Self {
            name: s.name.as_str().to_owned(),
            cpus: s.cpus,
            memory_gib: s.memory_gib,
            disk_gib: s.disk_gib,
            vm_type: s.vm_type,
            mount_type: s.mount_type,
            runtime: s.runtime,
            rosetta: s.rosetta,
            network_address: s.network_address,
            mounts: s.mounts.clone(),
            kubernetes: s.kubernetes,
        }
    }

    fn spec(&self, name: ProfileName) -> ProfileSpec {
        ProfileSpec {
            name,
            cpus: self.cpus,
            memory_gib: self.memory_gib,
            disk_gib: self.disk_gib,
            vm_type: self.vm_type,
            mount_type: self.mount_type,
            runtime: self.runtime,
            rosetta: self.rosetta,
            network_address: self.network_address,
            mounts: self.mounts.clone(),
            kubernetes: self.kubernetes,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Create,
    Edit { current: ProfileSpec },
}

/// Emitted to the parent so it can close the form.
#[derive(Debug, Clone, PartialEq)]
pub enum FormEvent {
    Saved(ProfileName),
    Cancelled,
    /// A create passed the rules and started: the parent shows `progress` for the new
    /// profile instead of a frozen form (F1). `Saved` follows only if the form stays open.
    Creating {
        spec: ProfileSpec,
        progress: Entity<StartProgress>,
    },
}

enum Msg {
    Done(Result<(), AppError>),
}

type Inbox = Arc<Mutex<Vec<Msg>>>;

pub struct ProfileEditor {
    app: Telima,
    os: HostOs,
    mode: Mode,
    draft: Draft,
    violations: Vec<RuleViolation>,
    name_error: Option<String>,
    /// After the first submit, every change re-checks so messages clear as the user fixes them.
    checked: bool,
    saving: bool,
    save_error: Option<String>,
    /// Progress of a running create.
    creating: Option<(Entity<StartProgress>, Subscription)>,
    inbox: Inbox,
    name_focus: FocusHandle,
    /// One per `draft.mounts` row, same order.
    mount_focus: Vec<FocusHandle>,
    _poll: Task<()>,
}

impl EventEmitter<FormEvent> for ProfileEditor {}

impl ProfileEditor {
    /// Form for a new profile on `os`.
    pub fn create(app: Telima, os: HostOs, cx: &mut Context<Self>) -> Self {
        Self::with(app, os, Mode::Create, Draft::new_for(os), cx)
    }

    /// Form for an existing profile; `current` is its spec (`VmManager::spec`).
    pub fn edit(app: Telima, os: HostOs, current: ProfileSpec, cx: &mut Context<Self>) -> Self {
        let draft = Draft::from_spec(&current);
        Self::with(app, os, Mode::Edit { current }, draft, cx)
    }

    fn with(app: Telima, os: HostOs, mode: Mode, draft: Draft, cx: &mut Context<Self>) -> Self {
        let mount_focus = draft.mounts.iter().map(|_| cx.focus_handle()).collect();
        Self {
            app,
            os,
            mode,
            draft,
            violations: Vec::new(),
            name_error: None,
            checked: false,
            saving: false,
            save_error: None,
            creating: None,
            inbox: Inbox::default(),
            name_focus: cx.focus_handle(),
            mount_focus,
            _poll: super::poll(cx, Self::pump),
        }
    }

    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    pub fn draft(&self) -> &Draft {
        &self.draft
    }

    pub fn name(&self) -> &str {
        &self.draft.name
    }

    pub fn is_saving(&self) -> bool {
        self.saving
    }

    /// Progress of the create started by the last submit.
    pub fn creating(&self) -> Option<&Entity<StartProgress>> {
        self.creating.as_ref().map(|(p, _)| p)
    }

    /// True when the field cannot change: `rules::IMMUTABLE_FIELDS` on an existing profile.
    pub fn is_locked(&self, field: Field) -> bool {
        matches!(self.mode, Mode::Edit { .. })
            && rules::IMMUTABLE_FIELDS
                .iter()
                .any(|(key, _)| *key == field.key())
    }

    pub fn vm_options(&self) -> &'static [VmType] {
        rules::vm_types(self.os)
    }

    pub fn mount_options(&self) -> &'static [MountType] {
        rules::mount_types(self.draft.vm_type)
    }

    /// Rosetta can be turned on with the current VM type (asked of `rules`, not repeated here).
    pub fn rosetta_available(&self) -> bool {
        let probe = ProfileSpec {
            rosetta: true,
            ..self.draft.spec(placeholder_name())
        };
        !matches!(rules::validate_new(&probe, self.os), Err(e) if e.contains(&RuleViolation::RosettaUnavailable))
    }

    /// Smallest disk the stepper offers: the current size when editing.
    pub fn min_disk(&self) -> u32 {
        match &self.mode {
            Mode::Edit { current } => current.disk_gib,
            Mode::Create => 1,
        }
    }

    /// Inline messages for one field.
    pub fn violations_for(&self, field: Field) -> Vec<String> {
        let mut out: Vec<String> = self
            .violations
            .iter()
            .filter(|v| Field::of(v) == field)
            .map(violation_text)
            .collect();
        if field == Field::Name
            && let Some(e) = &self.name_error
        {
            out.insert(0, e.clone());
        }
        out
    }

    pub fn has_violations(&self) -> bool {
        !self.violations.is_empty() || self.name_error.is_some()
    }

    fn change(&mut self, field: Field, cx: &mut Context<Self>, apply: impl FnOnce(&mut Draft)) {
        if self.is_locked(field) || self.saving {
            return;
        }
        apply(&mut self.draft);
        if self.checked {
            self.check();
        }
        cx.notify();
    }

    pub fn set_name(&mut self, name: &str, cx: &mut Context<Self>) {
        self.change(Field::Name, cx, |d| d.name = name.to_owned());
    }

    pub fn set_cpus(&mut self, n: u32, cx: &mut Context<Self>) {
        self.change(Field::Cpus, cx, |d| d.cpus = n);
    }

    pub fn set_memory(&mut self, gib: u32, cx: &mut Context<Self>) {
        self.change(Field::Memory, cx, |d| d.memory_gib = gib);
    }

    pub fn set_disk(&mut self, gib: u32, cx: &mut Context<Self>) {
        self.change(Field::Disk, cx, |d| d.disk_gib = gib);
    }

    /// Changing the VM type also moves the mount type and Rosetta to values it allows.
    pub fn set_vm_type(&mut self, vm: VmType, cx: &mut Context<Self>) {
        let os = self.os;
        self.change(Field::VmType, cx, |d| {
            d.vm_type = vm;
            if !rules::mount_types(vm).contains(&d.mount_type) {
                d.mount_type = rules::mount_types(vm)[0];
            }
            if d.rosetta {
                let probe = d.spec(placeholder_name());
                if rules::validate_new(&probe, os)
                    .is_err_and(|e| e.contains(&RuleViolation::RosettaUnavailable))
                {
                    d.rosetta = false;
                }
            }
        });
    }

    pub fn set_mount_type(&mut self, m: MountType, cx: &mut Context<Self>) {
        self.change(Field::MountType, cx, |d| d.mount_type = m);
    }

    pub fn set_rosetta(&mut self, on: bool, cx: &mut Context<Self>) {
        self.change(Field::Rosetta, cx, |d| d.rosetta = on);
    }

    pub fn set_network_address(&mut self, on: bool, cx: &mut Context<Self>) {
        self.change(Field::NetworkAddress, cx, |d| d.network_address = on);
    }

    /// Adds an empty read-only mount row (up to the row cap).
    pub fn add_mount(&mut self, cx: &mut Context<Self>) {
        if self.draft.mounts.len() >= MOUNT_SWITCH_IDS.len() || self.saving {
            return;
        }
        self.mount_focus.push(cx.focus_handle());
        self.change(Field::Mounts, cx, |d| {
            d.mounts.push(Mount {
                path: String::new(),
                writable: false,
            })
        });
    }

    pub fn remove_mount(&mut self, i: usize, cx: &mut Context<Self>) {
        if i >= self.draft.mounts.len() || self.saving {
            return;
        }
        self.mount_focus.remove(i);
        self.change(Field::Mounts, cx, |d| {
            d.mounts.remove(i);
        });
    }

    pub fn set_mount_path(&mut self, i: usize, path: &str, cx: &mut Context<Self>) {
        self.change(Field::Mounts, cx, |d| {
            if let Some(m) = d.mounts.get_mut(i) {
                m.path = path.to_owned();
            }
        });
    }

    pub fn set_mount_writable(&mut self, i: usize, writable: bool, cx: &mut Context<Self>) {
        self.change(Field::Mounts, cx, |d| {
            if let Some(m) = d.mounts.get_mut(i) {
                m.writable = writable;
            }
        });
    }

    pub fn set_kubernetes(&mut self, on: bool, cx: &mut Context<Self>) {
        self.change(Field::Kubernetes, cx, |d| d.kubernetes = on);
    }

    /// Edit only: which saved changes need a restart (`rules::restart_required`), as app copy.
    pub fn restart_hint(&self) -> Option<String> {
        let Mode::Edit { current } = &self.mode else {
            return None;
        };
        let next = self.draft.spec(current.name.clone());
        restart_required_text(&rules::restart_required(current, &next))
    }

    /// Runs the rules. Returns the spec when it may be sent.
    fn check(&mut self) -> Option<ProfileSpec> {
        let parsed = ProfileName::parse(self.draft.name.trim());
        self.name_error = parsed.as_ref().err().map(|_| {
            if self.draft.name.trim().is_empty() {
                "이름을 입력하세요".to_owned()
            } else {
                format!(
                    "영문 소문자, 숫자, '-'만 쓸 수 있습니다 (최대 {}자)",
                    ProfileName::MAX_LEN
                )
            }
        });
        let spec = self
            .draft
            .spec(parsed.clone().unwrap_or_else(|_| placeholder_name()));
        let result = match &self.mode {
            Mode::Create => rules::validate_new(&spec, self.os),
            Mode::Edit { current } => rules::validate_update(current, &spec, self.os),
        };
        self.violations = result.err().unwrap_or_default();
        (parsed.is_ok() && self.violations.is_empty()).then_some(spec)
    }

    /// Validates and, only when the rules pass, calls create or update.
    pub fn submit(&mut self, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.checked = true;
        self.save_error = None;
        let Some(spec) = self.check() else {
            cx.notify();
            return;
        };
        self.saving = true;
        match self.mode {
            Mode::Create => {
                let app = self.app.clone();
                let run = spec.clone();
                let progress = cx.new(|cx| StartProgress::create(app, run, cx));
                let sub = cx.subscribe(&progress, |v, _, end: &ProgressEnd, cx| {
                    v.saving = false;
                    match end {
                        ProgressEnd::Done => {
                            if let Ok(name) = ProfileName::parse(v.draft.name.trim()) {
                                cx.emit(FormEvent::Saved(name));
                            }
                        }
                        ProgressEnd::Failed(e) => v.save_error = Some(e.clone()),
                        ProgressEnd::Cancelled => {}
                    }
                    cx.notify();
                });
                self.creating = Some((progress.clone(), sub));
                cx.emit(FormEvent::Creating { spec, progress });
            }
            Mode::Edit { .. } => {
                let inbox = self.inbox.clone();
                self.app
                    .update_profile(spec, move |r| inbox.lock().unwrap().push(Msg::Done(r)));
            }
        }
        cx.notify();
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        cx.emit(FormEvent::Cancelled);
    }

    /// Applies finished use-case results. Called by the poll task (and by tests).
    pub fn pump(&mut self, cx: &mut Context<Self>) {
        if let Some(progress) = self.creating().cloned() {
            progress.update(cx, |p, cx| p.pump(cx));
        }
        let msgs = std::mem::take(&mut *self.inbox.lock().unwrap());
        if msgs.is_empty() {
            return;
        }
        for msg in msgs {
            match msg {
                Msg::Done(result) => {
                    self.saving = false;
                    match result {
                        Ok(()) => {
                            if let Ok(name) = ProfileName::parse(self.draft.name.trim()) {
                                cx.emit(FormEvent::Saved(name));
                            }
                        }
                        Err(e) => self.save_error = Some(error_text(&e)),
                    }
                }
            }
        }
        cx.notify();
    }
}

/// Stand-in name so the other rules can run while the typed name is still invalid.
fn placeholder_name() -> ProfileName {
    ProfileName::parse("x").expect("valid placeholder")
}

/// A number with − and + buttons.
struct Stepper {
    id: &'static str,
    value: u32,
    unit: &'static str,
    min: u32,
    set: fn(&mut ProfileEditor, u32, &mut Context<ProfileEditor>),
}

/// One button per allowed option.
struct Choices<T: 'static> {
    id: &'static str,
    options: &'static [T],
    selected: T,
    label: fn(T) -> &'static str,
    locked: bool,
    set: fn(&mut ProfileEditor, T, &mut Context<ProfileEditor>),
}

impl ProfileEditor {
    fn stepper(&self, s: Stepper, cx: &mut Context<Self>, p: &crate::theme::Palette) -> Div {
        let Stepper {
            id,
            value,
            unit,
            min,
            set,
        } = s;
        let can_dec = !self.saving && value > min;
        let can_inc = !self.saving;
        h_flex()
            .gap(space(1))
            .child(
                button(
                    SharedString::from(format!("{id}-dec")),
                    "−",
                    can_dec,
                    Tone::Default,
                    p,
                )
                .when(can_dec, |b| {
                    b.on_click(cx.listener(move |v, _, _, cx| set(v, value - 1, cx)))
                }),
            )
            .child(
                div()
                    .min_w(px(metric::FIELD_NUM))
                    .text_color(hsla(p.text))
                    .text_size(px(font::BODY.size))
                    .font_family(font::MONO)
                    .child(format!("{value} {unit}")),
            )
            .child(
                button(
                    SharedString::from(format!("{id}-inc")),
                    "+",
                    can_inc,
                    Tone::Default,
                    p,
                )
                .when(can_inc, |b| {
                    b.on_click(cx.listener(move |v, _, _, cx| set(v, value + 1, cx)))
                }),
            )
    }

    fn choices<T: Copy + PartialEq + 'static>(
        &self,
        c: Choices<T>,
        cx: &mut Context<Self>,
        p: &crate::theme::Palette,
    ) -> Div {
        let Choices {
            id,
            options,
            selected,
            label,
            locked,
            set,
        } = c;
        let enabled = !locked && !self.saving;
        let labels = options
            .iter()
            .map(|&o| SharedString::from(label(o)))
            .collect();
        let index = options.iter().position(|&o| o == selected).unwrap_or(0);
        let this = cx.entity().downgrade();
        // A locked choice still shows which value is set; clicks do nothing.
        segmented(id, labels, index, p, move |i, _, cx| {
            if enabled {
                let _ = this.update(cx, |v, cx| set(v, options[i], cx));
            }
        })
    }

    fn toggle(
        &self,
        id: &'static str,
        on: bool,
        enabled: bool,
        set: fn(&mut Self, bool, &mut Context<Self>),
        cx: &mut Context<Self>,
        p: &crate::theme::Palette,
    ) -> Stateful<Div> {
        let enabled = enabled && !self.saving;
        let this = cx.entity().downgrade();
        // An unavailable switch keeps its state visible; the field hint says why.
        switch(id, on, p, move |next, _, cx| {
            if enabled {
                let _ = this.update(cx, |v, cx| set(v, next, cx));
            }
        })
    }
}

impl ProfileEditor {
    /// Path box, writable switch and remove button per mount, then the add button.
    fn mounts_editor(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
        p: &crate::theme::Palette,
    ) -> Div {
        let enabled = !self.saving;
        let rows = self.draft.mounts.iter().enumerate().map(|(i, m)| {
            let focus = self.mount_focus[i].clone();
            let path_box = text_box(
                SharedString::from(format!("mount-path-{i}")),
                &m.path,
                "/Users/me/src",
                !enabled,
                &focus,
                window,
                p,
            )
            .flex_1()
            .debug_selector({
                let id = format!("mount-path-{i}");
                move || id.clone()
            })
            .when(enabled, |b| {
                b.on_click(cx.listener(move |_, _, window, cx| window.focus(&focus, cx)))
                    .on_key_down(cx.listener(move |v, ev: &KeyDownEvent, _, cx| {
                        let mut text = v.draft.mounts[i].path.clone();
                        if edit_text(&mut text, ev) {
                            v.set_mount_path(i, &text, cx);
                        }
                    }))
            });
            let this = cx.entity().downgrade();
            let writable = switch(MOUNT_SWITCH_IDS[i], m.writable, p, move |next, _, cx| {
                if enabled {
                    let _ = this.update(cx, |v, cx| v.set_mount_writable(i, next, cx));
                }
            });
            let mode = telima_app::view::daily::mount_rows(std::slice::from_ref(m))[0].mode;
            let remove_id = format!("mount-remove-{i}");
            h_flex()
                .gap(space(1))
                .child(path_box)
                .child(writable)
                .child(
                    div()
                        .min_w(px(metric::FIELD_NUM))
                        .text_size(px(font::SMALL.size))
                        .text_color(hsla(p.text_2))
                        .child(mode),
                )
                .child(
                    button(
                        SharedString::from(remove_id.clone()),
                        "−",
                        enabled,
                        Tone::Default,
                        p,
                    )
                    .debug_selector(move || remove_id.clone())
                    .when(enabled, |b| {
                        b.on_click(cx.listener(move |v, _, _, cx| v.remove_mount(i, cx)))
                    }),
                )
        });
        let can_add = enabled && self.draft.mounts.len() < MOUNT_SWITCH_IDS.len();
        v_flex()
            .flex_1()
            .gap(space(1))
            .when(self.draft.mounts.is_empty(), |d| {
                d.child(hint_line(MOUNTS_EMPTY, p))
            })
            .children(rows)
            .child(
                h_flex().child(
                    button("mount-add", "+ 추가", can_add, Tone::Default, p)
                        .debug_selector(|| "mount-add".to_string())
                        .when(can_add, |b| {
                            b.on_click(cx.listener(|v, _, _, cx| v.add_mount(cx)))
                        }),
                ),
            )
    }
}

impl Render for ProfileEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(window);
        let editing = matches!(self.mode, Mode::Edit { .. });
        let name_locked = self.is_locked(Field::Name);
        let name_box = text_box(
            "profile-name",
            &self.draft.name,
            "예: dev",
            name_locked,
            &self.name_focus,
            window,
            p,
        )
        .w(px(metric::FIELD_SELECT))
        .debug_selector(|| "profile-name".to_string())
        .when(!name_locked, |b| {
            b.on_click(cx.listener(|v, _, window, cx| window.focus(&v.name_focus, cx)))
                .on_key_down(cx.listener(|v, ev: &KeyDownEvent, _, cx| {
                    let mut text = v.draft.name.clone();
                    if edit_text(&mut text, ev) {
                        v.set_name(&text, cx);
                    } else if ev.keystroke.key == "enter" {
                        v.submit(cx);
                    }
                }))
        });
        let lock_hint = |f: Field| self.is_locked(f).then_some("만든 뒤에는 바꿀 수 없습니다");
        let d = self.draft.clone();
        let k8s = kubernetes_copy(d.runtime);

        let rows = v_flex()
            .gap(space(2))
            .child(field_row(
                "이름",
                name_box,
                lock_hint(Field::Name),
                self.violations_for(Field::Name),
                p,
            ))
            .child(field_row(
                "CPU",
                self.stepper(
                    Stepper {
                        id: "cpus",
                        value: d.cpus,
                        unit: "개",
                        min: 1,
                        set: Self::set_cpus,
                    },
                    cx,
                    p,
                ),
                None,
                self.violations_for(Field::Cpus),
                p,
            ))
            .child(field_row(
                "메모리",
                self.stepper(
                    Stepper {
                        id: "memory",
                        value: d.memory_gib,
                        unit: "GiB",
                        min: 1,
                        set: Self::set_memory,
                    },
                    cx,
                    p,
                ),
                None,
                self.violations_for(Field::Memory),
                p,
            ))
            .child(field_row(
                "디스크",
                self.stepper(
                    Stepper {
                        id: "disk",
                        value: d.disk_gib,
                        unit: "GiB",
                        min: self.min_disk(),
                        set: Self::set_disk,
                    },
                    cx,
                    p,
                ),
                editing.then_some("늘리기만 할 수 있습니다"),
                self.violations_for(Field::Disk),
                p,
            ))
            .child(field_row(
                "VM 종류",
                self.choices(
                    Choices {
                        id: "vm",
                        options: self.vm_options(),
                        selected: d.vm_type,
                        label: vm_label,
                        locked: self.is_locked(Field::VmType),
                        set: Self::set_vm_type,
                    },
                    cx,
                    p,
                ),
                lock_hint(Field::VmType),
                self.violations_for(Field::VmType),
                p,
            ))
            .child(field_row(
                "마운트",
                self.choices(
                    Choices {
                        id: "mount",
                        options: self.mount_options(),
                        selected: d.mount_type,
                        label: mount_label,
                        locked: self.is_locked(Field::MountType),
                        set: Self::set_mount_type,
                    },
                    cx,
                    p,
                ),
                None,
                self.violations_for(Field::MountType),
                p,
            ))
            .child(field_row(
                "Rosetta",
                self.toggle(
                    "rosetta",
                    d.rosetta,
                    self.rosetta_available(),
                    Self::set_rosetta,
                    cx,
                    p,
                ),
                (!self.rosetta_available()).then_some("macOS의 vz VM에서만 쓸 수 있습니다"),
                self.violations_for(Field::Rosetta),
                p,
            ))
            .child(field_row(
                "IP 주소",
                self.toggle(
                    "network-address",
                    d.network_address,
                    true,
                    Self::set_network_address,
                    cx,
                    p,
                ),
                Some("VM에 호스트에서 닿는 IP 주소를 줍니다"),
                self.violations_for(Field::NetworkAddress),
                p,
            ))
            .child(field_row(
                "디렉터리",
                self.mounts_editor(window, cx, p),
                Some(MOUNTS_HELP),
                self.violations_for(Field::Mounts),
                p,
            ))
            .child(field_row(
                "Kubernetes",
                self.toggle(
                    "kubernetes",
                    d.kubernetes,
                    k8s.available,
                    Self::set_kubernetes,
                    cx,
                    p,
                ),
                Some(k8s.disabled_reason.unwrap_or(k8s.description)),
                self.violations_for(Field::Kubernetes),
                p,
            ))
            .children(
                self.violations_for(Field::Runtime)
                    .into_iter()
                    .enumerate()
                    .map(|(i, e)| error_line(format!("runtime-error-{i}"), e, p)),
            );

        let submit_label = match (editing, self.saving) {
            (_, true) => "저장 중…",
            (false, false) => "만들기",
            (true, false) => "저장",
        };
        let footer = h_flex()
            .gap(space(1))
            .justify_end()
            .child(
                button("form-cancel", "취소", !self.saving, Tone::Default, p)
                    .debug_selector(|| "form-cancel".to_string())
                    .when(!self.saving, |b| {
                        b.on_click(cx.listener(|v, _, _, cx| v.cancel(cx)))
                    }),
            )
            .child(
                button("form-submit", submit_label, !self.saving, Tone::Primary, p)
                    .debug_selector(|| "form-submit".to_string())
                    .when(!self.saving, |b| {
                        b.on_click(cx.listener(|v, _, _, cx| v.submit(cx)))
                    }),
            );

        // N4: inline in the shell's space(4) inset, no frame; the shell crumb (create) or
        // profile header (edit) is the only title.
        let frame = v_flex()
            .debug_selector(|| "profile-form-frame".to_string())
            // L7: label column + gap + widest field, so the footer's right edge is the fields'.
            .w(px(metric::FIELD_LABEL
                + metric::SPACE[3]
                + metric::FIELD_SELECT))
            .gap(space(2))
            .text_color(hsla(p.text))
            .font_family(font::SANS)
            .child(rows)
            .children(
                self.restart_hint()
                    .map(|h| hint_line(h, p).debug_selector(|| "restart-hint".to_string())),
            )
            .children(self.save_error.as_ref().map(|e| {
                div()
                    .p(space(2))
                    .rounded(px(metric::RADIUS_MD))
                    .border_1()
                    .border_color(hsla(p.err_border))
                    .bg(hsla(p.err_bg))
                    .text_color(hsla(p.err))
                    .text_size(px(font::SMALL.size))
                    .child(paragraph(format!("저장하지 못했습니다. {e}")))
            }))
            .child(footer);
        fade_in("profile-editor", frame)
    }
}
