//! Virgo's single-stroke, monochrome product icons.

use crate::SidebarView;
use std::sync::OnceLock;
use taim::widget::svg;

const EXPLORER: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M7 3h7l4 4v13H7z"/><path d="M14 3v5h4M4 7v14h11"/></svg>"##;
const SEARCH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round"><circle cx="10.5" cy="10.5" r="6.5"/><path d="m15.5 15.5 5 5"/></svg>"##;
const GIT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="10" r="2"/><path d="M6 7v10M6 12h7a5 5 0 0 0 5-5"/></svg>"##;
const DEBUG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="m8 5 10 7-10 7z"/><path d="M4 5h1M4 12h1M4 19h1"/></svg>"##;
const EXTENSIONS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><path d="M17.5 13.5v8M13.5 17.5h8"/></svg>"##;
const SETTINGS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M10 2h4l.6 2.4 2.1.9 2.1-1.3 2.8 2.8-1.3 2.1.9 2.1L23 12l-1.8 1 .1 2.1-2.5 2.5-2.1-1.2-2.1.9L14 20h-4l-.6-2.7-2.1-.9-2.1 1.2-2.5-2.5.9-2.1L2 12l1.6-1 .1-2.1-1-2.1 2.5-2.5 2.1 1.2 2.1-.9z"/></svg>"##;
const FILE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M5 2.5h9l5 5V21H5zM14 2.5v5h5"/></svg>"##;
const FOLDER: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M2.5 6h7l2 2H21v11H2.5z"/></svg>"##;
const TERMINAL: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="2.5" y="4" width="19" height="16" rx="2"/><path d="m6 9 3 3-3 3M11.5 15H17"/></svg>"##;
const BUILD: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M4 6h16M4 12h16M4 18h12"/></svg>"##;
const PLUS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="#b8b8b8" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M12 4v16M4 12h16"/></svg>"##;

pub fn activity(view: SidebarView) -> svg::Handle {
    static HANDLES: [OnceLock<svg::Handle>; 6] = [const { OnceLock::new() }; 6];
    let (index, bytes) = match view {
        SidebarView::Explorer => (0, EXPLORER),
        SidebarView::Search => (1, SEARCH),
        SidebarView::SourceControl => (2, GIT),
        SidebarView::Debug => (3, DEBUG),
        SidebarView::Extensions => (4, EXTENSIONS),
        SidebarView::Settings => (5, SETTINGS),
    };
    HANDLES[index]
        .get_or_init(|| svg::Handle::from_memory(bytes))
        .clone()
}

pub fn file(is_directory: bool) -> svg::Handle {
    static HANDLES: [OnceLock<svg::Handle>; 2] = [const { OnceLock::new() }; 2];
    let index = usize::from(is_directory);
    HANDLES[index]
        .get_or_init(|| svg::Handle::from_memory(if is_directory { FOLDER } else { FILE }))
        .clone()
}

pub fn terminal() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(TERMINAL))
        .clone()
}

pub fn build() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(BUILD))
        .clone()
}

pub fn plus() -> svg::Handle {
    static HANDLE: OnceLock<svg::Handle> = OnceLock::new();
    HANDLE
        .get_or_init(|| svg::Handle::from_memory(PLUS))
        .clone()
}
