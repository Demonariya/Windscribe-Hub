#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;
use windscribe_hub::app::AppState;
use windscribe_hub::models::NavSection;
use windscribe_hub::ui::components::dialogs::{
    ConfirmationDialog, ConfirmationSpec, DialogChoice, NotificationToast,
};
use windscribe_hub::ui::components::header::{NavigationMenu, WindscribeHeader};
use windscribe_hub::ui::components::window::WindscribeWindow;
use windscribe_hub::ui::metrics::{
    WINDOW_DEFAULT_HEIGHT, WINDOW_DEFAULT_WIDTH, WINDOW_MIN_HEIGHT, WINDOW_MIN_WIDTH,
};
use windscribe_hub::ui::screens;
use windscribe_hub::ui::theme::{apply_egui_visuals, ThemeTokens};

struct WindscribeHubApp {
    state: AppState,
    system_is_dark: bool,
    last_screen: NavSection,
}

impl WindscribeHubApp {
    fn new(system_is_dark: bool) -> Self {
        Self {
            state: AppState::new(),
            system_is_dark,
            last_screen: NavSection::Home,
        }
    }
}

impl eframe::App for WindscribeHubApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::TRANSPARENT.to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.state.poll_background_events();
        self.state.handle_keyboard_shortcuts(ctx);
        ctx.request_repaint_after(Duration::from_millis(200));

        let tokens = ThemeTokens::resolve(self.state.settings.theme, self.system_is_dark);
        apply_egui_visuals(ctx, &tokens);

        let title = concat!("Windscribe Hub  •  v", env!("CARGO_PKG_VERSION"));
        let anim_enabled = self.state.settings.animations_enabled;
        let busy = self.state.is_busy();

        WindscribeWindow::show(ctx, &tokens, title, |ui| {
            let hdr_action = WindscribeHeader::show(
                ui,
                ctx,
                &tokens,
                self.state.drawer_open,
                busy,
                anim_enabled,
                &self.state.status,
            );
            if hdr_action.toggle_drawer {
                self.state.drawer_open = !self.state.drawer_open;
            }
            if hdr_action.refresh_clicked {
                self.state.refresh_all_status();
            }
            if hdr_action.open_settings {
                self.state.active_section = NavSection::Settings;
                self.state.drawer_open = false;
            }

            if let Some(next_sec) = NavigationMenu::show_compact_strip(
                ui,
                ctx,
                &tokens,
                self.state.active_section,
                anim_enabled,
            ) {
                self.state.active_section = next_sec;
                self.state.drawer_open = false;
            }

            // Screen transition subtle horizontal settle
            if self.last_screen != self.state.active_section {
                self.last_screen = self.state.active_section;
                ctx.data_mut(|d| {
                    d.insert_temp(egui::Id::new("screen_enter_t"), 0.0_f32);
                });
            }
            let enter_t = if anim_enabled {
                ctx.animate_value_with_time(egui::Id::new("screen_enter_t"), 1.0, 0.16)
            } else {
                1.0
            };

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if enter_t < 0.99 {
                        ui.add_space((1.0 - enter_t) * 6.0);
                    }
                    match self.state.active_section {
                        NavSection::Home => screens::home::show(ui, ctx, &mut self.state, &tokens),
                        NavSection::Accounts => {
                            screens::accounts::show(ui, ctx, &mut self.state, &tokens)
                        }
                        NavSection::Tools => {
                            screens::tools::show(ui, ctx, &mut self.state, &tokens)
                        }
                        NavSection::Registry => {
                            screens::registry::show(ui, &mut self.state, &tokens)
                        }
                        NavSection::Settings => {
                            screens::settings::show(ui, ctx, &mut self.state, &tokens)
                        }
                    }
                });
        });

        // Sliding Windscribe-style navigation drawer
        if let Some(picked) = NavigationMenu::show_drawer(
            ctx,
            &tokens,
            &mut self.state.drawer_open,
            self.state.active_section,
            anim_enabled,
            self.state.accounts.len(),
            &self.state.status,
        ) {
            self.state.active_section = picked;
        }

        // Registry Cleanup Confirmation Dialog
        if let Some(target) = self.state.pending_registry_clean {
            match ConfirmationDialog::show(
                ctx,
                &tokens,
                ConfirmationSpec {
                    id_salt: "confirm_registry_clean",
                    title: "Confirm Registry Cleanup",
                    body: "Windscribe Hub will verify the key exists, export a timestamped .reg backup, verify the backup file, and then delete this registry entry:",
                    highlight: Some(target.registry_path()),
                    confirm_label: "Back Up & Delete",
                    destructive: true,
                },
                anim_enabled,
            ) {
                DialogChoice::Confirmed => {
                    self.state.pending_registry_clean = None;
                    self.state.execute_registry_cleanup(target);
                }
                DialogChoice::Cancelled => {
                    self.state.pending_registry_clean = None;
                }
                DialogChoice::Pending => {}
            }
        }

        // Account Delete Confirmation Dialog
        if let Some(account_id) = self.state.pending_delete_account.clone() {
            let username = self
                .state
                .accounts
                .iter()
                .find(|a| a.id == account_id)
                .map(|a| a.username.clone())
                .unwrap_or_else(|| "this account".to_owned());
            match ConfirmationDialog::show(
                ctx,
                &tokens,
                ConfirmationSpec {
                    id_salt: "confirm_account_delete",
                    title: "Delete Saved Account",
                    body: "Remove these encrypted credentials from your Windows DPAPI vault?",
                    highlight: Some(&username),
                    confirm_label: "Delete Account",
                    destructive: true,
                },
                anim_enabled,
            ) {
                DialogChoice::Confirmed => {
                    self.state.pending_delete_account = None;
                    self.state.confirm_delete_account(&account_id);
                }
                DialogChoice::Cancelled => {
                    self.state.pending_delete_account = None;
                }
                DialogChoice::Pending => {}
            }
        }

        NotificationToast::show_stack(ctx, &tokens, &mut self.state.toasts, anim_enabled);
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_DEFAULT_WIDTH, WINDOW_DEFAULT_HEIGHT])
            .with_min_inner_size([WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT])
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(true)
            .with_title("Windscribe Hub"),
        ..Default::default()
    };

    eframe::run_native(
        "Windscribe Hub",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let system_is_dark = cc.egui_ctx.style().visuals.dark_mode;
            Ok(Box::new(WindscribeHubApp::new(system_is_dark)))
        }),
    )
}
