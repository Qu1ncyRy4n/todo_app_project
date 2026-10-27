# Emoji rendering in the graph editor

Updated: 2026-10-26

The missing-box issue was not a ledger encoding problem. The editor used
eframe/egui 0.36.2, whose bundled outline fonts lacked many emoji glyphs.
egui did not automatically use the operating system's fallback fonts in that
version. Adding Apple Symbols.ttf filled some monochrome symbols but could not
rasterize the color-only Apple emoji font through the outline-font path.

The selected fix is eframe 0.37.0. Its native integration enables system-font
fallback by default and can load color emoji from installed fonts. We removed
the Apple Symbols override so it cannot claim a glyph ahead of the system's
emoji font. This changes rendering only, not task text or stored events.

Primary sources:

- [egui 0.37 release: emoji and system glyph support](https://github.com/emilk/egui/releases/tag/0.37.0)
- [eframe 0.37 system-font feature and runtime option](https://docs.rs/eframe/0.37.0/eframe/)
- [egui 0.37 minimum Rust version](https://docs.rs/egui/0.37.0/egui/)

The pinned toolchain is Rust 1.97.1, above egui's Rust 1.95 minimum. Automated
checks can establish build and text preservation, but cannot prove how Apple
Color Emoji and compound ZWJ sequences appear in the installed window. Test
plain emoji and multi-codepoint emoji in a task title, its card, the new-task
field, search results, and Info before closing the acceptance item.
