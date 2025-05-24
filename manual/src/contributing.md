# Contributing

Welcome! Thank you for interest in the `system-manager` project, your contributions are greatly appreciated.

The following is a general guide for contributing to the project. For guidance on extending `system-manager` to fit specific needs,
such as adding support for your distribution, or creating a release branch, see the [Extending System Manager](./contributing/extending-system-manager.md) section of the manual.

## Getting Started

> `system-manager` development requires a nix installation with the `flakes` and `nix-command` features enabled. If you do not have nix installed, please refer to the [Installation](./installation.md) section of the manual.

1. Firstly, [create a fork the repository](github.com/numtide/system-manager/fork) where you will make your changes.
1. Create a copy of your newly created fork on your local machine: `git clone git@github.com:<USER>/system-manager.git`, where `<USER>` is which ever account the fork was created on.
1. Enter the development environment: `nix develop`. This will supply you with the tool necessary to build and test the repository.
1. [Create an issue](#creating-issues) for the problem you are trying to solve, if it does not already exist.
1. [Create a pull request](#creating-pull-requests) that would close the issue.

### Creating Pull Requests

> Important: Please be sure an issue exists for the problem you are trying to fix before opening a pull request.

1. Create a working branch that targets the issue number you would like to close: `git checkout -b <USER>/123`.
1. Add, commit and push your changes:

```sh
git add -A
git commit -m "fix: Fixes ..."
git push origin <USER>/123
```

3. [Open a pull request upstream](https://github.com/numtide/system-manager/compare) for your branch targetting the `main` branch.
1. Please add a few sentences which describe your changes, and use [closing keywords](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/linking-a-pull-request-to-an-issue) to close the issue your pull request aims to close automatically.

### Creating Issues

Before creating a new issue, please do a [quick search of the existing issues](github.com/numtide/system-manager/issues) to be sure the problem is not already being tracked or worked on by someone else.
