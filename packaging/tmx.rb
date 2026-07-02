# Homebrew formula for tmx.
#
# This file belongs in a Homebrew tap repository named `homebrew-tmx`
# (i.e. github.com/krunalparmarem/homebrew-tmx) as `Formula/tmx.rb`, so users can:
#
#   brew install krunalparmarem/tmx/tmx
#
# On each release, bump `url` to the new tag and update `sha256`
# (`curl -sL <url> | shasum -a 256`).
class Tmx < Formula
  desc "10x agentic tmux workspace for AI developers"
  homepage "https://github.com/krunalparmarem/tmx"
  url "https://github.com/krunalparmarem/tmx/archive/refs/tags/v0.3.0.tar.gz"
  sha256 "REPLACE_WITH_TARBALL_SHA256"
  license "MIT"
  head "https://github.com/krunalparmarem/tmx.git", branch: "main"

  depends_on "rust" => :build
  depends_on "tmux"

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match "tmx", shell_output("#{bin}/tmx --version")
  end
end
