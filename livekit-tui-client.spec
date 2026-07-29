Name:           livekit-tui-client
Version:        0.1.0
Release:        1%{?dist}
Summary:        Terminal-based LiveKit client with Zig/Odin video rendering

License:        MIT
URL:            https://github.com/username/livekit-tui-client
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  zig
BuildRequires:  odin
BuildRequires:  alsa-lib-devel
Requires:       glibc
Requires:       alsa-lib

%description
A terminal-based LiveKit client built with Rust and Ratatui, featuring 
True Color video rendering using Odin (pixel animation) and Zig (mosaic).

%prep
%autosetup

%build
cargo build --release --bin client

%install
rm -rf $RPM_BUILD_ROOT
install -D -m 0755 target/release/client $RPM_BUILD_ROOT/%{_bindir}/livekit-tui-client

%files
%{_bindir}/livekit-tui-client

%changelog
* Wed Jul 29 2026 Your Name <your.email@example.com> - 0.1.0-1
- Initial release with UI configuration and dual rendering modes.
