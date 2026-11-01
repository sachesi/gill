%define _debugsource_template %{nil}
%define debug_package %{nil}


Name:           gill
# The release workflow sets Version to the tag it builds; OBS counts the Release.
Version:        0.1.0
Release:        0
Summary:        Run the bash lines fish cannot, and keep bash's state between them

License:        GPL-3.0-or-later
URL:            https://github.com/sachesi/gill
# Named as the Debian source package names them, which OBS builds from the same files.
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz#/%{name}_%{version}.orig.tar.gz
# The crates the build needs, from the release, so that it runs without a network.
Source1:        %{url}/releases/download/v%{version}/%{name}-%{version}-vendor.tar.xz#/%{name}_%{version}.orig-vendor.tar.xz

BuildRequires:  cargo >= 1.88
BuildRequires:  rust >= 1.88
BuildRequires:  gcc

Requires:       bash >= 5.0
Requires:       fish >= 4.0

%global _description %{expand:
gill sends a line typed at the fish prompt to bash when fish cannot run it, such
as a bash loop or an assignment. Exported variables and the working directory
come back into fish, and bash's variables, functions, aliases and options last
from one line to the next. Nothing changes until a user loads it from their fish
configuration.}

%description %{_description}

%prep
%autosetup -n %{name}-%{version} -b 1

%build
export CARGO_HOME=$PWD/.cargo-home
export RUSTFLAGS="%{?build_rustflags}"
cargo build --release --offline --locked

%install
install -Dpm 0755 target/release/gill %{buildroot}%{_bindir}/gill

%check
test -x %{buildroot}%{_bindir}/gill

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/gill

%changelog
