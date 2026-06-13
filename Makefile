# Makefile for building Windows .exe from Linux

.PHONY: windows clean-patches icon build clean help

# Default target
windows: patch-gpui icon build
	@echo ""
	@echo "=== Build successful ==="
	@echo "Output: target/x86_64-pc-windows-msvc/debug/dicto.exe"
	@ls -lh target/x86_64-pc-windows-msvc/debug/dicto.exe | awk '{print "Size:   " $$5}'

# Apply gpui patches (fixes cross-compilation issues)
patch-gpui:
	@echo "Applying gpui patches..."
	@bash patch-gpui-build.sh || true
	@echo "Done"

# Generate Windows icon from SVG
icon:
	@echo "Generating Windows icon..."
	@printf '%s\n%s\n' "32512 ICON \"icon.ico\"" "// Windows resource script" > gpui/icon.rc
	@if ! command -v rsvg-convert >/dev/null 2>&1; then \
		echo "ERROR: rsvg-convert is required for transparent icons (librsvg)." >&2; \
		exit 1; \
	fi
	@rsvg-convert -w 256 -h 256 -f png assets/icon.svg -o tmp_256.png
	@rsvg-convert -w 128 -h 128 -f png assets/icon.svg -o tmp_128.png
	@rsvg-convert -w 64 -h 64 -f png assets/icon.svg -o tmp_64.png
	@rsvg-convert -w 32 -h 32 -f png assets/icon.svg -o tmp_32.png
	@rsvg-convert -w 16 -h 16 -f png assets/icon.svg -o tmp_16.png
	@magick tmp_256.png tmp_128.png tmp_64.png tmp_32.png tmp_16.png -alpha on -define icon:auto-resize=256,128,64,32,16 gpui/icon.ico
	@rm -f tmp_*.png
	@file gpui/icon.ico | cut -d: -f2- | head -c 80

# Build Windows .exe
build:
	@echo "Building Windows .exe..."
	@cargo clean -p dicto --target x86_64-pc-windows-msvc
	@cargo xwin build --target x86_64-pc-windows-msvc -p dicto

# Clean build artifacts
clean:
	@echo "Cleaning..."
	@rm -f gpui/icon.rc gpui/icon.ico
	@cargo clean -p dicto --target x86_64-pc-windows-msvc
	@echo "Cleaned Windows build artifacts"

# Reset gpui patches (useful after cargo update)
reset-patches:
	@echo "Resetting gpui patches..."
	@bash -c 'cd $$(find ~/.cargo/git/checkouts/zed-*/ae47ec9 -maxdepth 0 -type d 2>/dev/null | head -1) && git checkout -- crates/gpui/build.rs crates/gpui_windows/build.rs crates/gpui_windows/src/directx_renderer.rs'
	@echo "Patches reset. Run 'make windows' to apply and rebuild."

# Show help
help:
	@echo "Available targets:"
	@echo "  make windows      - Build Windows .exe (default)"
	@echo "  make patch-gpui   - Apply gpui cross-compilation patches only"
	@echo "  make icon         - Generate Windows icon only"
	@echo "  make build        - Build .exe (requires patches and icon)"
	@echo "  make clean        - Clean Windows build artifacts"
	@echo "  make reset-patches- Reset gpui patches (use after cargo update)"
	@echo "  make help         - Show this help"
	@echo ""
	@echo "Example workflow:"
	@echo "  1. make windows           # Full build (patches + icon + compile)"
	@echo "  2. make icon && make build  # Generate icon and build separately"