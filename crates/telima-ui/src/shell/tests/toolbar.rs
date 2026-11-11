//! U1 window toolbar (canvas 07-09): profile dropdown, drag area, lifecycle buttons or
//! progress text, divider, refresh, new profile. The header and sidebar carry no actions.
use gpui_kit::{AppContext, px};
use telima_app::AppCommand;
use telima_app::view::StatusKind;
use telima_app::view::tray::{NEW_PROFILE, REFRESH, toolbar_actions};
use telima_core::profile::ProfileName;

use super::{Fixture, click, fixture, parts, real_window};
use crate::components::icons::{Icon, strokes};
use crate::shell::{MenuRow, Screen, lifecycle_icon};
use crate::theme::metric;

fn name(raw: &str) -> ProfileName {
    ProfileName::parse(raw).unwrap()
}

fn lifecycle(f: &Fixture, cx: &mut gpui_kit::TestAppContext) -> Vec<AppCommand> {
    f.view.read_with(cx, |v, _| {
        v.toolbar_actions()
            .lifecycle
            .into_iter()
            .map(|i| i.command)
            .collect()
    })
}

fn list_calls(calls: &[String]) -> usize {
    calls.iter().filter(|c| *c == "list").count()
}

#[gpui_kit::test]
fn u1_toolbar_shows_only_state_valid_lifecycle_buttons(cx: &mut gpui_kit::TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    assert_eq!(
        f.view.read_with(cx, |v, _| v.toolbar_actions()),
        toolbar_actions(Some((&name("default"), StatusKind::Running)))
    );
    assert_eq!(
        lifecycle(&f, cx),
        [
            AppCommand::Restart(name("default")),
            AppCommand::Stop(name("default"))
        ]
    );
    assert_eq!(f.view.read_with(cx, |v, cx| v.toolbar_progress(cx)), None);
    f.view.update(cx, |v, cx| v.select("work", cx));
    assert_eq!(lifecycle(&f, cx), [AppCommand::Start(name("work"))]);
    // While the command runs: no buttons, the progress status with its elapsed time.
    f.view.update(cx, |v, cx| v.start(cx));
    assert!(lifecycle(&f, cx).is_empty());
    let progress = f
        .view
        .read_with(cx, |v, cx| v.toolbar_progress(cx))
        .expect("progress text while starting");
    let status = f
        .view
        .read_with(cx, |v, cx| v.header_status_label(cx))
        .unwrap();
    assert!(progress.starts_with(&status), "{progress}");
    assert!(progress.ends_with("00:00"), "{progress}");
}

#[gpui_kit::test]
fn u1_refresh_and_new_profile_are_always_offered(cx: &mut gpui_kit::TestAppContext) {
    let f = fixture(cx);
    let a = f.view.read_with(cx, |v, _| v.toolbar_actions());
    assert_eq!(
        (a.refresh.label.as_str(), a.refresh.command),
        (REFRESH, AppCommand::Refresh)
    );
    assert_eq!(
        (a.new_profile.label.as_str(), a.new_profile.command),
        (NEW_PROFILE, AppCommand::NewProfile)
    );
}

#[gpui_kit::test]
fn u1_refresh_reads_the_profile_list_again(cx: &mut gpui_kit::TestAppContext) {
    let f = fixture(cx);
    let before = list_calls(&f.vm.calls());
    f.view
        .update(cx, |v, cx| v.run_command(&AppCommand::Refresh, cx));
    f.settle(cx);
    assert_eq!(list_calls(&f.vm.calls()), before + 1);
}

#[gpui_kit::test]
fn u1_new_profile_command_opens_the_create_form(cx: &mut gpui_kit::TestAppContext) {
    let f = fixture(cx);
    f.view
        .update(cx, |v, cx| v.run_command(&AppCommand::NewProfile, cx));
    assert_eq!(f.view.read_with(cx, |v, _| v.screen()), Screen::Create);
    assert!(lifecycle(&f, cx).is_empty());
}

#[gpui_kit::test]
fn u1_lifecycle_commands_run_on_the_named_profile(cx: &mut gpui_kit::TestAppContext) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    f.view.update(cx, |v, cx| {
        v.run_command(&AppCommand::Start(name("work")), cx)
    });
    assert_eq!(
        f.view.read_with(cx, |v, _| v.selected().cloned()),
        Some(name("work"))
    );
    f.settle(cx);
    assert!(f.vm.calls().contains(&"start work".to_owned()));
}

#[gpui_kit::test]
fn u1_profile_menu_lists_profiles_with_status_and_checks_the_current_one(
    cx: &mut gpui_kit::TestAppContext,
) {
    let f = fixture(cx);
    f.view.update(cx, |v, cx| v.select("default", cx));
    assert_eq!(f.view.read_with(cx, |v, _| v.profile_menu()), None);
    f.view.update(cx, |v, cx| v.toggle_profile_menu(cx));
    let row = |n: &str, kind, current| MenuRow {
        name: n.to_owned(),
        kind,
        current,
    };
    assert_eq!(
        f.view.read_with(cx, |v, _| v.profile_menu()),
        Some(vec![
            row("alpha", StatusKind::Stopped, false),
            row("default", StatusKind::Running, true),
            row("work", StatusKind::Stopped, false),
        ])
    );
    // Picking a row selects it and closes the menu.
    f.view.update(cx, |v, cx| v.pick_profile("work", cx));
    assert_eq!(
        f.view.read_with(cx, |v, _| v.selected().cloned()),
        Some(name("work"))
    );
    assert_eq!(f.view.read_with(cx, |v, _| v.profile_menu()), None);
    // Toggling twice closes it; 새 프로필… opens the form and closes it.
    f.view.update(cx, |v, cx| v.toggle_profile_menu(cx));
    f.view.update(cx, |v, cx| v.toggle_profile_menu(cx));
    assert_eq!(f.view.read_with(cx, |v, _| v.profile_menu()), None);
    f.view.update(cx, |v, cx| v.toggle_profile_menu(cx));
    f.view
        .update(cx, |v, cx| v.run_command(&AppCommand::NewProfile, cx));
    assert_eq!(f.view.read_with(cx, |v, _| v.profile_menu()), None);
    assert_eq!(f.view.read_with(cx, |v, _| v.screen()), Screen::Create);
}

#[test]
fn u1_lifecycle_buttons_carry_canvas_icons_and_refresh_has_its_own() {
    let n = name("default");
    assert_eq!(lifecycle_icon(&AppCommand::Start(n.clone())), Icon::Play);
    assert_eq!(lifecycle_icon(&AppCommand::Stop(n.clone())), Icon::Stop);
    assert_eq!(lifecycle_icon(&AppCommand::Restart(n)), Icon::Restart);
    assert_eq!(lifecycle_icon(&AppCommand::Refresh), Icon::Refresh);
    assert_eq!(lifecycle_icon(&AppCommand::NewProfile), Icon::Plus);
    assert_ne!(strokes(Icon::Refresh), strokes(Icon::Restart));
}

#[test]
fn u1_toolbar_code_has_no_korean_literals() {
    let src = include_str!("../toolbar.rs");
    let hangul =
        |c: char| ('\u{AC00}'..='\u{D7A3}').contains(&c) || ('\u{3131}'..='\u{318E}').contains(&c);
    for (i, line) in src.lines().enumerate() {
        let code = line.split("//").next().unwrap_or_default();
        assert!(!code.chars().any(hangul), "toolbar.rs:{}: {line}", i + 1);
    }
}

/// Real window with the profile list loaded and `default` selected.
fn real_default(
    cx: &mut gpui_kit::TestAppContext,
) -> (
    gpui_kit::Entity<crate::shell::MainView>,
    std::sync::Arc<telima_app::testing::FakeVm>,
    std::sync::Arc<super::QueueSpawner>,
    &mut gpui_kit::VisualTestContext,
) {
    let (vm, spawner, _, app) = parts(|_| {});
    let (view, cx) = real_window(app, cx);
    spawner.run_all();
    view.update(cx, |v, cx| v.pump(cx));
    view.update(cx, |v, cx| v.select("default", cx));
    spawner.run_all();
    view.update(cx, |v, cx| v.pump(cx));
    cx.run_until_parked();
    (view, vm, spawner, cx)
}

#[gpui_kit::test]
fn u1_real_window_toolbar_holds_every_action_once(cx: &mut gpui_kit::TestAppContext) {
    let (view, vm, spawner, cx) = real_default(cx);
    assert!(
        cx.debug_bounds("sidebar-new-profile").is_none(),
        "the sidebar + moved to the toolbar"
    );
    for sel in [
        "toolbar-profile",
        "toolbar-drag",
        "toolbar-restart",
        "toolbar-stop",
        "toolbar-divider",
        "toolbar-refresh",
        "toolbar-new-profile",
    ] {
        assert!(cx.debug_bounds(sel).is_some(), "{sel} rendered");
    }
    assert!(cx.debug_bounds("toolbar-start").is_none());
    // The empty space between the dropdown and the buttons is the drag area, full height.
    let profile = cx.debug_bounds("toolbar-profile").unwrap();
    let drag = cx.debug_bounds("toolbar-drag").unwrap();
    let restart = cx.debug_bounds("toolbar-restart").unwrap();
    assert!(drag.size.width > px(0.));
    assert!(drag.left() >= profile.right() && drag.right() <= restart.left());
    assert_eq!(drag.size.height, px(metric::TITLEBAR));

    // Refresh reads the list again.
    let before = list_calls(&vm.calls());
    click(cx, "toolbar-refresh");
    spawner.run_all();
    view.update(cx, |v, cx| v.pump(cx));
    assert_eq!(list_calls(&vm.calls()), before + 1);

    // The dropdown picks another profile; its buttons follow.
    click(cx, "toolbar-profile");
    assert!(view.read_with(cx, |v, _| v.profile_menu().is_some()));
    click(cx, "profile-menu-work");
    assert_eq!(
        view.read_with(cx, |v, _| v.selected().cloned()),
        Some(name("work"))
    );
    assert!(view.read_with(cx, |v, _| v.profile_menu().is_none()));
    assert!(cx.debug_bounds("toolbar-stop").is_none());

    // Starting swaps the buttons for the progress text.
    click(cx, "toolbar-start");
    assert!(cx.debug_bounds("toolbar-start").is_none());
    assert!(cx.debug_bounds("toolbar-progress").is_some());
}

#[gpui_kit::test]
fn u1_real_window_new_profile_from_toolbar_and_menu(cx: &mut gpui_kit::TestAppContext) {
    let (view, _, _, cx) = real_default(cx);
    click(cx, "toolbar-new-profile");
    assert_eq!(view.read_with(cx, |v, _| v.screen()), Screen::Create);
    view.update(cx, |v, cx| v.select("default", cx));
    cx.run_until_parked();
    click(cx, "toolbar-profile");
    click(cx, "profile-menu-new");
    assert_eq!(view.read_with(cx, |v, _| v.screen()), Screen::Create);
    assert!(view.read_with(cx, |v, _| v.profile_menu().is_none()));
}
