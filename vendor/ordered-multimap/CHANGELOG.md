# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

# 0.7.1 - 10-24-2023

### Changed

 - Updated `coverage-helper` dependency to `0.2.0`.

# 0.7.0 - 08-23-2023

### Changed

 - Updated `hashbrown` dependency to `0.14.0`

# 0.6.0 - 01-21-2023

### Added

 - Added support for `no_std` - @notgull

### Changed

 - Updated `dlv-list` dependency to `0.5.0`.
 - Updated `hashbrown` dependency to `0.13.2`.

# 0.5.0 - 08-25-2022

### Changed

 - Loosened bounds required on some functions.
 - Updated `dlv-list` dependency to `0.4.0`.

### Fixed

 - `serde` implementation now correctly works as a multimap.

# 0.4.3

### Changed

 - Updated `hashbrown` dependency to `0.12.0`.

# 0.4.2

### Changed

 - Updated `dlv-list` dependency to `0.3.0`. This is not a breaking change as it's not user visible.

# 0.4.1

### Changed

 - Updated `dlv-list` dependency to `0.2.4`.
 - Updated `hashbrown` dependency to `0.11.0`.

# 0.4.0

### Removed

 - Removed `drain_pairs` as it's unsafe.

### Fixed

 - Fixed miri issues with `retain`.

# 0.3.1

### Added

 - Added crate feature `serde` for (de)serialization.
 - Implemented `IntoIterator` of owned key-value pairs for `ListOrderedMultimap`.

# 0.3.0

### Changed

 - Updated `hashbrown` dependency to `0.9.0`.

# 0.2.4

### Changed

 - Updated `dlv-list` dependency to `0.2.2`.
 - Updated `hashbrown` dependency to `0.7.0`.

# 0.2.3

### Changed

 - Works on stable Rust.
 - Updated `hashbrown` dependency to `0.6.0`.

# 0.2.2

### Fixed

 - Fix crate as it was broken from std's migration to hashbrown.

# 0.2.1

### Changed

 - Update dependency on `dlv-list` which will reduce memory size of `ListOrderedMultimap` by 48 bytes.

# 0.2.0

### Added

 - Initial release.

# 0.1.0

### Removed

 - Version was yanked due to critical design flaw.
