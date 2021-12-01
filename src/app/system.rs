use crate::app::ui::{key, key_val, val};
use crate::app::App;
use crate::event::SystemInspectInfo;

use egui::{CollapsingHeader, Grid};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CentralView {
    Home,
    DataUsage,
}

impl Default for CentralView {
    fn default() -> Self {
        CentralView::Home
    }
}

#[derive(Default, Debug)]
pub struct SystemTab {
    pub system_info: Option<Box<SystemInspectInfo>>,
    pub central_view: CentralView,
}

impl App {
    pub fn system_view(&mut self, ui: &mut egui::Ui) {
        match self.system.central_view {
            CentralView::Home => self.system_details(ui),
            CentralView::DataUsage => self.system_data_usage(ui),
        }
    }

    pub fn system_side(&mut self, ui: &mut egui::Ui) {
        self.system_menu(ui);
    }

    fn system_menu(&mut self, ui: &mut egui::Ui) {
        Grid::new("system_menu").show(ui, |ui| {
            ui.selectable_value(&mut self.system.central_view, CentralView::Home, "home");
            ui.selectable_value(
                &mut self.system.central_view,
                CentralView::DataUsage,
                "data usage",
            );
        });
    }

    fn system_data_usage(&mut self, ui: &mut egui::Ui) {
        ui.label("TODO!");
    }

    fn system_details(&mut self, ui: &mut egui::Ui) {
        if let Some(system) = &self.system.system_info {
            Grid::new("basic_info_grid").show(ui, |ui| {
                key_val!(ui, "Version:", &system.version.version);
                key_val!(ui, "API version:", &system.version.api_version);
                key_val!(ui, "OS type:", &system.info.os_type);
                if !system.info.operating_system.is_empty() {
                    key_val!(ui, "OS:", &system.info.operating_system);
                }
                if !system.info.os_version.is_empty() {
                    key_val!(ui, "OS version:", &system.info.os_version);
                }
                key_val!(ui, "Architecture:", &system.version.arch);
                key_val!(ui, "Kernel version:", &system.version.kernel_version);
                key_val!(ui, "Go version:", &system.version.go_version);
                key_val!(ui, "Git commit:", &system.version.git_commit);
                key_val!(ui, "Build time:", system.version.build_time.to_rfc2822());

                if !system.info.labels.is_empty() {
                    key!(ui, "Labels:");
                    ui.end_row();
                    ui.label("          ");
                    Grid::new("labels_grid").show(ui, |ui| {
                        let mut labels = system.info.labels.iter().collect::<Vec<_>>();
                        labels.sort();
                        for label in labels {
                            val!(ui, &label);
                            ui.end_row();
                        }
                    });
                    ui.end_row();
                }
            });

            CollapsingHeader::new("details")
                .default_open(false)
                .show(ui, |ui| {
                    Grid::new("details_grid").show(ui, |ui| {
                        key_val!(ui, "Experimental build:", system.info.experimental_build);
                        key_val!(ui, "Debug:", system.info.debug);
                        key_val!(ui, "Driver:", &system.info.driver);
                        key_val!(ui, "Logging driver:", &system.info.logging_driver);
                        key_val!(ui, "CGroup driver:", &system.info.cgroup_driver);
                        key_val!(ui, "CGroup version:", &system.info.cgroup_version);
                        key_val!(ui, "Init binary:", &system.info.init_binary);
                        key_val!(ui, "Root directory:", &system.info.docker_root_dir);
                        key_val!(ui, "Isolation:", system.info.isolation.as_ref());
                        key_val!(ui, "Pids limit:", system.info.pids_limit);
                        if let Some(license) = system.info.product_license.as_ref() {
                            key_val!(ui, "License:", &license);
                        }
                        if !system.info.security_options.is_empty() {
                            key!(ui, "Security options:");
                            ui.end_row();
                            ui.label("          ");
                            Grid::new("sec_opts_grid").show(ui, |ui| {
                                let mut opts =
                                    system.info.security_options.iter().collect::<Vec<_>>();
                                opts.sort();
                                for opt in opts {
                                    val!(ui, &opt);
                                    ui.end_row();
                                }
                            });
                            ui.end_row();
                        }
                    });
                    CollapsingHeader::new("CPU")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("cpus_grid").show(ui, |ui| {
                                key_val!(ui, "CPUs:", system.info.n_cpu);
                                key_val!(ui, "CPU CFS period:", system.info.cpu_cfs_period);
                                key_val!(ui, "CPU CFS quota:", system.info.cpu_cfs_quota);
                                key_val!(ui, "CPU shares:", system.info.cpu_shares);
                                key_val!(ui, "CPU set:", system.info.cpu_set);
                            });
                        });
                    CollapsingHeader::new("memory")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("mem_grid").show(ui, |ui| {
                                key_val!(ui, "Total memory:", system.info.mem_total);
                                key_val!(ui, "Memory limit:", system.info.memory_limit);
                                key_val!(ui, "Swap limit:", system.info.swap_limit);
                                key_val!(ui, "OOM kill disable:", system.info.oom_kill_disable);
                            });
                        });
                    CollapsingHeader::new("network")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("net_grid").show(ui, |ui| {
                                key_val!(ui, "HTTP proxy:", &system.info.http_proxy);
                                key_val!(ui, "HTTPS proxy:", &system.info.https_proxy);
                                key_val!(ui, "No proxy:", &system.info.no_proxy);
                                key_val!(ui, "IPv4 forwarding:", system.info.ipv4_forwarding);
                                key_val!(ui, "Bridge nf iptables:", system.info.bridge_nf_iptables);
                                key_val!(
                                    ui,
                                    "Bridge nf ip6tables:",
                                    system.info.bridge_nf_ip6tables
                                );
                            });
                        });
                    CollapsingHeader::new("counters")
                        .default_open(false)
                        .show(ui, |ui| {
                            Grid::new("counters_grid").show(ui, |ui| {
                                key_val!(ui, "Total containers:", system.info.containers);
                                key_val!(ui, "Running containers:", system.info.containers_running);
                                key_val!(ui, "Paused containers:", system.info.containers_paused);
                                key_val!(ui, "Stopped containers:", system.info.containers_stopped);
                                key_val!(ui, "Images:", system.info.images);
                                key_val!(ui, "File descriptors:", system.info.n_fd);
                                key_val!(ui, "Go routines:", system.info.n_goroutines);
                                key_val!(ui, "Event listeners:", system.info.n_goroutines);
                            });
                        });
                });
        }
    }
}
