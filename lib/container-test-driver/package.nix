{
  extraPythonPackages ? (_ps: [ ]),
  python3Packages,
  python3,
  util-linux,
  systemd,
  mkShell,
  iproute2,
}:
let
  package = python3Packages.buildPythonApplication {
    pname = "container-test-driver";
    version = "0.0.1";
    propagatedBuildInputs = [
      util-linux
      systemd
      iproute2
      python3Packages.colorama
      python3Packages.junit-xml
      python3Packages.ptpython
      python3Packages.ipython
      python3Packages.pytest-testinfra
    ]
    ++ extraPythonPackages python3Packages;
    nativeBuildInputs = [ python3Packages.setuptools ];
    nativeCheckInputs = [
      python3Packages.pytest
      python3Packages.pytest-mypy
    ];
    format = "pyproject";
    src = ./.;
    passthru.devShell = mkShell {
      packages = [
        (python3.withPackages (_ps: package.propagatedBuildInputs))
        package.propagatedBuildInputs
        python3.pkgs.pytest
      ];
      shellHook = ''
        export PYTHONPATH="$(realpath .):$PYTHONPATH"
      '';
    };
  };
in
package
