class BashAst < Formula
  desc "Parse bash scripts to JSON AST using GNU Bash's actual parser"
  homepage "https://github.com/cv/bash-ast"
  url "https://github.com/cv/bash-ast.git",
      tag:      "VERSION_PLACEHOLDER",
      revision: "REVISION_PLACEHOLDER"
  license "GPL-3.0-only"
  head "https://github.com/cv/bash-ast.git", branch: "main"

  depends_on "llvm" => :build
  depends_on "rust" => :build

  def install
    # Ensure LLVM is found by bindgen
    ENV["LLVM_CONFIG_PATH"] = Formula["llvm"].opt_bin/"llvm-config"

    # Initialize and update git submodules (bash source)
    system "git", "submodule", "update", "--init", "--recursive"

    # Build and install the release binary
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Test basic parsing
    output = pipe_output(bin/"bash-ast", "echo hello")
    json = JSON.parse(output)
    assert_equal "simple", json["type"]

    # Test pipeline parsing
    output = pipe_output(bin/"bash-ast", "ls | grep foo")
    json = JSON.parse(output)
    assert_equal "pipeline", json["type"]

    # Test for loop parsing
    output = pipe_output(bin/"bash-ast", "for i in a b c; do echo $i; done")
    json = JSON.parse(output)
    assert_equal "for", json["type"]
    assert_equal "i", json["variable"]
  end
end
