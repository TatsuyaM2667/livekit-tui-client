pkgname=livekit-tui-client
pkgver=0.1.0
pkgrel=1
pkgdesc="Terminal-based LiveKit client with Zig/Odin video rendering"
arch=('x86_64')
url="https://github.com/username/livekit-tui-client"
license=('MIT')
depends=('glibc' 'gcc-libs' 'alsa-lib')
makedepends=('cargo' 'zig' 'odin')
source=("local://.") # Placeholder for source tarball
md5sums=('SKIP')

build() {
    cd "$srcdir"
    cargo build --release --bin client
}

package() {
    cd "$srcdir"
    install -Dm755 "target/release/client" "$pkgdir/usr/bin/livekit-tui-client"
}
