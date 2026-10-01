// The MIT License (MIT)

// Copyright (c) 2014 Y. T. CHUNG

// Permission is hereby granted, free of charge, to any person obtaining a copy of
// this software and associated documentation files (the "Software"), to deal in
// the Software without restriction, including without limitation the rights to
// use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software is furnished to do so,
// subject to the following conditions:

// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
// FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
// COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
// IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! Ini parser for Rust
//!
//! ```no_run
//! use ini::Ini;
//!
//! let mut conf = Ini::new();
//! conf.with_section(Some("User"))
//!     .set("name", "Raspberry树莓")
//!     .set("value", "Pi");
//! conf.with_section(Some("Library"))
//!     .set("name", "Sun Yat-sen U")
//!     .set("location", "Guangzhou=world");
//! conf.write_to_file("conf.ini").unwrap();
//!
//! let i = Ini::load_from_file("conf.ini").unwrap();
//! for (sec, prop) in i.iter() {
//!     println!("Section: {:?}", sec);
//!     for (k, v) in prop.iter() {
//!         println!("{}:{}", k, v);
//!     }
//! }
//! ```

use std::{
    borrow::Cow,
    char,
    error,
    fmt::{self, Display},
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    ops::{Index, IndexMut},
    path::Path,
    str::Chars,
};

use cfg_if::cfg_if;
use ordered_multimap::{
    list_ordered_multimap::{Entry, IntoIter, Iter, IterMut, OccupiedEntry, VacantEntry},
    ListOrderedMultimap,
};
#[cfg(feature = "case-insensitive")]
use unicase::UniCase;

/// Policies for escaping logic
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum EscapePolicy {
    /// Escape absolutely nothing (dangerous)
    Nothing,
    /// Only escape the most necessary things.
    /// This means backslashes, control characters (codepoints U+0000 to U+001F), and delete (U+007F).
    /// Quotes (single or double) are not escaped.
    Basics,
    /// Escape basics and non-ASCII characters in the [Basic Multilingual Plane](https://www.compart.com/en/unicode/plane)
    /// (i.e. between U+007F - U+FFFF)
    /// Codepoints above U+FFFF, e.g. '🐱' U+1F431 "CAT FACE" will *not* be escaped!
    BasicsUnicode,
    /// Escape basics and all non-ASCII characters, including codepoints above U+FFFF.
    /// This will escape emoji - if you want them to remain raw, use BasicsUnicode instead.
    BasicsUnicodeExtended,
    /// Escape reserved symbols.
    /// This includes everything in EscapePolicy::Basics, plus the comment characters ';' and '#' and the key/value-separating characters '=' and ':'.
    Reserved,
    /// Escape reserved symbols and non-ASCII characters in the BMP.
    /// Codepoints above U+FFFF, e.g. '🐱' U+1F431 "CAT FACE" will *not* be escaped!
    ReservedUnicode,
    /// Escape reserved symbols and all non-ASCII characters, including codepoints above U+FFFF.
    ReservedUnicodeExtended,
    /// Escape everything that some INI implementations assume
    Everything,
}

impl EscapePolicy {
    fn escape_basics(self) -> bool {
        self != EscapePolicy::Nothing
    }

    fn escape_reserved(self) -> bool {
        matches!(
            self,
            EscapePolicy::Reserved
                | EscapePolicy::ReservedUnicode
                | EscapePolicy::ReservedUnicodeExtended
                | EscapePolicy::Everything
        )
    }

    fn escape_unicode(self) -> bool {
        matches!(
            self,
            EscapePolicy::BasicsUnicode
                | EscapePolicy::BasicsUnicodeExtended
                | EscapePolicy::ReservedUnicode
                | EscapePolicy::ReservedUnicodeExtended
                | EscapePolicy::Everything
        )
    }

    fn escape_unicode_extended(self) -> bool {
        matches!(
            self,
            EscapePolicy::BasicsUnicodeExtended | EscapePolicy::ReservedUnicodeExtended | EscapePolicy::Everything
        )
    }

    /// Given a character this returns true if it should be escaped as
    /// per this policy or false if not.
    pub fn should_escape(self, c: char) -> bool {
        match c {
            // A single backslash, must be escaped
            // ASCII control characters, U+0000 NUL..= U+001F UNIT SEPARATOR, or U+007F DELETE. The same as char::is_ascii_control()
            '\\' | '\x00'..='\x1f' | '\x7f' => self.escape_basics(),
            ';' | '#' | '=' | ':' => self.escape_reserved(),
            '\u{0080}'..='\u{FFFF}' => self.escape_unicode(),
            '\u{10000}'..='\u{10FFFF}' => self.escape_unicode_extended(),
            _ => false,
        }
    }
}

// Escape non-INI characters
//
// Common escape sequences: https://en.wikipedia.org/wiki/INI_file#Escape_characters
//
// * `\\` \ (a single backslash, escaping the escape character)
// * `\0` Null character
// * `\a` Bell/Alert/Audible
// * `\b` Backspace, Bell character for some applications
// * `\t` Tab character
// * `\r` Carriage return
// * `\n` Line feed
// * `\;` Semicolon
// * `\#` Number sign
// * `\=` Equals sign
// * `\:` Colon
// * `\x????` Unicode character with hexadecimal code point corresponding to ????
fn escape_str(s: &str, policy: EscapePolicy) -> String {
    let mut escaped: String = String::with_capacity(s.len());
    for c in s.chars() {
        // if we know this is not something to escape as per policy, we just
        // write it and continue.
        if !policy.should_escape(c) {
            escaped.push(c);
            continue;
        }

        match c {
            '\\' => escaped.push_str("\\\\"),
            '\0' => escaped.push_str("\\0"),
            '\x01'..='\x06' | '\x0e'..='\x1f' | '\x7f'..='\u{00ff}' => {
                escaped.push_str(&format!("\\x{:04x}", c as isize)[..])
            }
            '\x07' => escaped.push_str("\\a"),
            '\x08' => escaped.push_str("\\b"),
            '\x0c' => escaped.push_str("\\f"),
            '\x0b' => escaped.push_str("\\v"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            '\u{0080}'..='\u{FFFF}' => escaped.push_str(&format!("\\x{:04x}", c as isize)[..]),
            // Longer escapes.
            '\u{10000}'..='\u{FFFFF}' => escaped.push_str(&format!("\\x{:05x}", c as isize)[..]),
            '\u{100000}'..='\u{10FFFF}' => escaped.push_str(&format!("\\x{:06x}", c as isize)[..]),
            _ => {
                escaped.push('\\');
                escaped.push(c);
            }
        }
    }
    escaped
}

/// Parsing configuration
pub struct ParseOption {
    /// Allow quote (`"` or `'`) in value
    /// For example
    /// ```ini
    /// [Section]
    /// Key1="Quoted value"
    /// Key2='Single Quote' with extra value
    /// ```
    ///
    /// In this example, Value of `Key1` is `Quoted value`,
    /// and value of `Key2` is `Single Quote with extra value`
    /// if `enabled_quote` is set to `true`.
    pub enabled_quote: bool,

    /// Interpret `\` as an escape character
    /// For example
    /// ```ini
    /// [Section]
    /// Key1=C:\Windows
    /// ```
    ///
    /// If `enabled_escape` is true, then the value of `Key` will become `C:Windows` (`\W` equals to `W`).
    pub enabled_escape: bool,

    /// Enables values that span lines
    /// ```ini
    /// [Section]
    /// foo=
    ///   b
    ///   c
    /// ```
    pub enabled_indented_mutiline_value: bool,

    /// Preserve key leading whitespace
    ///
    /// ```ini
    /// [services my-services]
    /// dynamodb=
    ///   endpoint_url=http://localhost:8000
    /// ```
    ///
    /// The leading whitespace in key `  endpoint_url` will be preserved if `enabled_preserve_key_leading_whitespace` is set to `true`.
    pub enabled_preserve_key_leading_whitespace: bool,
}

impl Default for ParseOption {
    fn default() -> ParseOption {
        ParseOption {
            enabled_quote: true,
            enabled_escape: true,
            enabled_indented_mutiline_value: false,
            enabled_preserve_key_leading_whitespace: false,
        }
    }
}

/// Newline style
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LineSeparator {
    /// System-dependent line separator
    ///
    /// On UNIX system, uses "\n"
    /// On Windows system, uses "\r\n"
    SystemDefault,

    /// Uses "\n" as new line separator
    CR,

    /// Uses "\r\n" as new line separator
    CRLF,
}

#[cfg(not(windows))]
static DEFAULT_LINE_SEPARATOR: &str = "\n";

#[cfg(windows)]
static DEFAULT_LINE_SEPARATOR: &str = "\r\n";

static DEFAULT_KV_SEPARATOR: &str = "=";

impl fmt::Display for LineSeparator {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(self.as_str())
    }
}

impl LineSeparator {
    /// String representation
    pub fn as_str(self) -> &'static str {
        match self {
            LineSeparator::SystemDefault => DEFAULT_LINE_SEPARATOR,
            LineSeparator::CR => "\n",
            LineSeparator::CRLF => "\r\n",
        }
    }
}

/// Writing configuration
#[derive(Debug, Clone)]
pub struct WriteOption {
    /// Policies about how to escape characters
    pub escape_policy: EscapePolicy,

    /// Newline style
    pub line_separator: LineSeparator,

    /// Key value separator
    pub kv_separator: &'static str,
}

impl Default for WriteOption {
    fn default() -> WriteOption {
        WriteOption {
            escape_policy: EscapePolicy::Basics,
            line_separator: LineSeparator::SystemDefault,
            kv_separator: DEFAULT_KV_SEPARATOR,
        }
    }
}

cfg_if! {
    if #[cfg(feature = "case-insensitive")] {
        /// Internal storage of section's key
        pub type SectionKey = Option<UniCase<String>>;
        /// Internal storage of property's key
        pub type PropertyKey = UniCase<String>;

        macro_rules! property_get_key {
            ($s:expr) => {
                &UniCase::from($s)
            };
        }

        macro_rules! property_insert_key {
            ($s:expr) => {
                UniCase::from($s)
            };
        }

        macro_rules! section_key {
            ($s:expr) => {
                $s.map(|s| UniCase::from(s.into()))
            };
        }

    } else {
        /// Internal storage of section's key
        pub type SectionKey = Option<String>;
        /// Internal storage of property's key
        pub type PropertyKey = String;

        macro_rules! property_get_key {
            ($s:expr) => {
                $s
            };
        }

        macro_rules! property_insert_key {
            ($s:expr) => {
                $s
            };
        }

        macro_rules! section_key {
            ($s:expr) => {
                $s.map(Into::into)
            };
        }
    }
}

/// A setter which could be used to set key-value pair in a specified section
pub struct SectionSetter<'a> {
    ini: &'a mut Ini,
    section_name: Option<String>,
}

impl<'a> SectionSetter<'a> {
    fn new(ini: &'a mut Ini, section_name: Option<String>) -> SectionSetter<'a> {
        SectionSetter { ini, section_name }
    }

    /// Set (replace) key-value pair in this section (all with the same name)
    pub fn set<'b, K, V>(&'b mut self, key: K, value: V) -> &'b mut SectionSetter<'a>
    where
        K: Into<String>,
        V: Into<String>,
        'a: 'b,
    {
        self.ini
            .entry(self.section_name.clone())
            .or_insert_with(Default::default)
            .insert(key, value);

        self
    }

    /// Add (append) key-value pair in this section
    pub fn add<'b, K, V>(&'b mut self, key: K, value: V) -> &'b mut SectionSetter<'a>
    where
        K: Into<String>,
        V: Into<String>,
        'a: 'b,
    {
        self.ini
            .entry(self.section_name.clone())
            .or_insert_with(Default::default)
            .append(key, value);

        self
    }

    /// Delete the first entry in this section with `key`
    pub fn delete<'b, K>(&'b mut self, key: &K) -> &'b mut SectionSetter<'a>
    where
        K: AsRef<str>,
        'a: 'b,
    {
        for prop in self.ini.section_all_mut(self.section_name.as_ref()) {
            prop.remove(key);
        }

        self
    }

    /// Get the entry in this section with `key`
    pub fn get<K: AsRef<str>>(&'a self, key: K) -> Option<&'a str> {
        self.ini
            .section(self.section_name.as_ref())
            .and_then(|prop| prop.get(key))
            .map(AsRef::as_ref)
    }
}

/// Properties type (key-value pairs)
#[derive(Clone, Default, Debug, PartialEq)]
pub struct Properties {
    data: ListOrderedMultimap<PropertyKey, String>,
}

impl Properties {
    /// Create an instance
    pub fn new() -> Properties {
        Default::default()
    }

    /// Get the number of the properties
    pub fn len(&self) -> usize {
        self.data.keys_len()
    }

    /// Check if properties has 0 elements
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get an iterator of the properties
    pub fn iter(&self) -> PropertyIter<'_> {
        PropertyIter {
            inner: self.data.iter(),
        }
    }

    /// Get a mutable iterator of the properties
    pub fn iter_mut(&mut self) -> PropertyIterMut<'_> {
        PropertyIterMut {
            inner: self.data.iter_mut(),
        }
    }

    /// Return true if property exist
    pub fn contains_key<S: AsRef<str>>(&self, s: S) -> bool {
        self.data.contains_key(property_get_key!(s.as_ref()))
    }

    /// Insert (key, value) pair by replace
    pub fn insert<K, V>(&mut self, k: K, v: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.data.insert(property_insert_key!(k.into()), v.into());
    }

    /// Append key with (key, value) pair
    pub fn append<K, V>(&mut self, k: K, v: V)
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.data.append(property_insert_key!(k.into()), v.into());
    }

    /// Get the first value associate with the key
    pub fn get<S: AsRef<str>>(&self, s: S) -> Option<&str> {
        self.data.get(property_get_key!(s.as_ref())).map(|v| v.as_str())
    }

    /// Get all values associate with the key
    pub fn get_all<S: AsRef<str>>(&self, s: S) -> impl DoubleEndedIterator<Item = &str> {
        self.data.get_all(property_get_key!(s.as_ref())).map(|v| v.as_str())
    }

    /// Remove the property with the first value of the key
    pub fn remove<S: AsRef<str>>(&mut self, s: S) -> Option<String> {
        self.data.remove(property_get_key!(s.as_ref()))
    }

    /// Remove the property with all values with the same key
    pub fn remove_all<S: AsRef<str>>(&mut self, s: S) -> impl DoubleEndedIterator<Item = String> + '_ {
        self.data.remove_all(property_get_key!(s.as_ref()))
    }

    fn get_mut<S: AsRef<str>>(&mut self, s: S) -> Option<&mut str> {
        self.data.get_mut(property_get_key!(s.as_ref())).map(|v| v.as_mut_str())
    }
}

impl<S: AsRef<str>> Index<S> for Properties {
    type Output = str;

    fn index(&self, index: S) -> &str {
        let s = index.as_ref();
        match self.get(s) {
            Some(p) => p,
            None => panic!("Key `{}` does not exist", s),
        }
    }
}

pub struct PropertyIter<'a> {
    inner: Iter<'a, PropertyKey, String>,
}

impl<'a> Iterator for PropertyIter<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (k.as_ref(), v.as_ref()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for PropertyIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|(k, v)| (k.as_ref(), v.as_ref()))
    }
}

/// Iterator for traversing sections
pub struct PropertyIterMut<'a> {
    inner: IterMut<'a, PropertyKey, String>,
}

impl<'a> Iterator for PropertyIterMut<'a> {
    type Item = (&'a str, &'a mut String);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (k.as_ref(), v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for PropertyIterMut<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|(k, v)| (k.as_ref(), v))
    }
}

pub struct PropertiesIntoIter {
    inner: IntoIter<PropertyKey, String>,
}

impl Iterator for PropertiesIntoIter {
    type Item = (String, String);

    #[cfg_attr(not(feature = "case-insensitive"), allow(clippy::useless_conversion))]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (k.into(), v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for PropertiesIntoIter {
    #[cfg_attr(not(feature = "case-insensitive"), allow(clippy::useless_conversion))]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|(k, v)| (k.into(), v))
    }
}

impl<'a> IntoIterator for &'a Properties {
    type IntoIter = PropertyIter<'a>;
    type Item = (&'a str, &'a str);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Properties {
    type IntoIter = PropertyIterMut<'a>;
    type Item = (&'a str, &'a mut String);

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl IntoIterator for Properties {
    type IntoIter = PropertiesIntoIter;
    type Item = (String, String);

    fn into_iter(self) -> Self::IntoIter {
        PropertiesIntoIter {
            inner: self.data.into_iter(),
        }
    }
}

/// A view into a vacant entry in a `Ini`
pub struct SectionVacantEntry<'a> {
    inner: VacantEntry<'a, SectionKey, Properties>,
}

impl<'a> SectionVacantEntry<'a> {
    /// Insert one new section
    pub fn insert(self, value: Properties) -> &'a mut Properties {
        self.inner.insert(value)
    }
}

/// A view into a occupied entry in a `Ini`
pub struct SectionOccupiedEntry<'a> {
    inner: OccupiedEntry<'a, SectionKey, Properties>,
}

impl<'a> SectionOccupiedEntry<'a> {
    /// Into the first internal mutable properties
    pub fn into_mut(self) -> &'a mut Properties {
        self.inner.into_mut()
    }

    /// Append a new section
    pub fn append(&mut self, prop: Properties) {
        self.inner.append(prop);
    }

    fn last_mut(&'a mut self) -> &'a mut Properties {
        self.inner
            .iter_mut()
            .next_back()
            .expect("occupied section shouldn't have 0 property")
    }
}

/// A view into an `Ini`, which may either be vacant or occupied.
pub enum SectionEntry<'a> {
    Vacant(SectionVacantEntry<'a>),
    Occupied(SectionOccupiedEntry<'a>),
}

impl<'a> SectionEntry<'a> {
    /// Ensures a value is in the entry by inserting the default if empty, and returns a mutable reference to the value in the entry.
    pub fn or_insert(self, properties: Properties) -> &'a mut Properties {
        match self {
            SectionEntry::Occupied(e) => e.into_mut(),
            SectionEntry::Vacant(e) => e.insert(properties),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty, and returns a mutable reference to the value in the entry.
    pub fn or_insert_with<F: FnOnce() -> Properties>(self, default: F) -> &'a mut Properties {
        match self {
            SectionEntry::Occupied(e) => e.into_mut(),
            SectionEntry::Vacant(e) => e.insert(default()),
        }
    }
}

impl<'a> From<Entry<'a, SectionKey, Properties>> for SectionEntry<'a> {
    fn from(e: Entry<'a, SectionKey, Properties>) -> SectionEntry<'a> {
        match e {
            Entry::Occupied(inner) => SectionEntry::Occupied(SectionOccupiedEntry { inner }),
            Entry::Vacant(inner) => SectionEntry::Vacant(SectionVacantEntry { inner }),
        }
    }
}

/// Ini struct
#[derive(Debug, Clone)]
pub struct Ini {
    sections: ListOrderedMultimap<SectionKey, Properties>,
}

impl Ini {
    /// Create an instance
    pub fn new() -> Ini {
        Default::default()
    }

    /// Set with a specified section, `None` is for the general section
    pub fn with_section<S>(&mut self, section: Option<S>) -> SectionSetter<'_>
    where
        S: Into<String>,
    {
        SectionSetter::new(self, section.map(Into::into))
    }

    /// Set with general section, a simple wrapper of `with_section(None::<String>)`
    pub fn with_general_section(&mut self) -> SectionSetter<'_> {
        self.with_section(None::<String>)
    }

    /// Get the immutable general section
    pub fn general_section(&self) -> &Properties {
        self.section(None::<String>)
            .expect("There is no general section in this Ini")
    }

    /// Get the mutable general section
    pub fn general_section_mut(&mut self) -> &mut Properties {
        self.section_mut(None::<String>)
            .expect("There is no general section in this Ini")
    }

    /// Get a immutable section
    pub fn section<S>(&self, name: Option<S>) -> Option<&Properties>
    where
        S: Into<String>,
    {
        self.sections.get(&section_key!(name))
    }

    /// Get a mutable section
    pub fn section_mut<S>(&mut self, name: Option<S>) -> Option<&mut Properties>
    where
        S: Into<String>,
    {
        self.sections.get_mut(&section_key!(name))
    }

    /// Get all sections immutable with the same key
    pub fn section_all<S>(&self, name: Option<S>) -> impl DoubleEndedIterator<Item = &Properties>
    where
        S: Into<String>,
    {
        self.sections.get_all(&section_key!(name))
    }

    /// Get all sections mutable with the same key
    pub fn section_all_mut<S>(&mut self, name: Option<S>) -> impl DoubleEndedIterator<Item = &mut Properties>
    where
        S: Into<String>,
    {
        self.sections.get_all_mut(&section_key!(name))
    }

    /// Get the entry
    #[cfg(not(feature = "case-insensitive"))]
    pub fn entry(&mut self, name: Option<String>) -> SectionEntry<'_> {
        SectionEntry::from(self.sections.entry(name))
    }

    /// Get the entry
    #[cfg(feature = "case-insensitive")]
    pub fn entry(&mut self, name: Option<String>) -> SectionEntry<'_> {
        SectionEntry::from(self.sections.entry(name.map(UniCase::from)))
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.sections.clear()
    }

    /// Iterate with sections
    pub fn sections(&self) -> impl DoubleEndedIterator<Item = Option<&str>> {
        self.sections.keys().map(|s| s.as_ref().map(AsRef::as_ref))
    }

    /// Set key-value to a section
    pub fn set_to<S>(&mut self, section: Option<S>, key: String, value: String)
    where
        S: Into<String>,
    {
        self.with_section(section).set(key, value);
    }

    /// Get the first value from the sections with key
    ///
    /// Example:
    ///
    /// ```
    /// use ini::Ini;
    /// let input = "[sec]\nabc = def\n";
    /// let ini = Ini::load_from_str(input).unwrap();
    /// assert_eq!(ini.get_from(Some("sec"), "abc"), Some("def"));
    /// ```
    pub fn get_from<'a, S>(&'a self, section: Option<S>, key: &str) -> Option<&'a str>
    where
        S: Into<String>,
    {
        self.sections.get(&section_key!(section)).and_then(|prop| prop.get(key))
    }

    /// Get the first value from the sections with key, return the default value if it does not exist
    ///
    /// Example:
    ///
    /// ```
    /// use ini::Ini;
    /// let input = "[sec]\n";
    /// let ini = Ini::load_from_str(input).unwrap();
    /// assert_eq!(ini.get_from_or(Some("sec"), "key", "default"), "default");
    /// ```
    pub fn get_from_or<'a, S>(&'a self, section: Option<S>, key: &str, default: &'a str) -> &'a str
    where
        S: Into<String>,
    {
        self.get_from(section, key).unwrap_or(default)
    }

    /// Get the first mutable value from the sections with key
    pub fn get_from_mut<'a, S>(&'a mut self, section: Option<S>, key: &str) -> Option<&'a mut str>
    where
        S: Into<String>,
    {
        self.sections
            .get_mut(&section_key!(section))
            .and_then(|prop| prop.get_mut(key))
    }

    /// Delete the first section with key, return the properties if it exists
    pub fn delete<S>(&mut self, section: Option<S>) -> Option<Properties>
    where
        S: Into<String>,
    {
        let key = section_key!(section);
        self.sections.remove(&key)
    }

    /// Delete the key from the section, return the value if key exists or None
    pub fn delete_from<S>(&mut self, section: Option<S>, key: &str) -> Option<String>
    where
        S: Into<String>,
    {
        self.section_mut(section).and_then(|prop| prop.remove(key))
    }

    /// Total sections count
    pub fn len(&self) -> usize {
        self.sections.keys_len()
    }

    /// Check if object contains no section
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

impl Default for Ini {
    /// Creates an ini instance with an empty general section. This allows [Ini::general_section]
    /// and [Ini::with_general_section] to be called without panicking.
    fn default() -> Self {
        let mut result = Ini {
            sections: Default::default(),
        };

        result.sections.insert(None, Default::default());

        result
    }
}

impl<S: Into<String>> Index<Option<S>> for Ini {
    type Output = Properties;

    fn index(&self, index: Option<S>) -> &Properties {
        match self.section(index) {
            Some(p) => p,
            None => panic!("Section does not exist"),
        }
    }
}

impl<S: Into<String>> IndexMut<Option<S>> for Ini {
    fn index_mut(&mut self, index: Option<S>) -> &mut Properties {
        match self.section_mut(index) {
            Some(p) => p,
            None => panic!("Section does not exist"),
        }
    }
}

impl<'q> Index<&'q str> for Ini {
    type Output = Properties;

    fn index<'a>(&'a self, index: &'q str) -> &'a Properties {
        match self.section(Some(index)) {
            Some(p) => p,
            None => panic!("Section `{}` does not exist", index),
        }
    }
}

impl<'q> IndexMut<&'q str> for Ini {
    fn index_mut<'a>(&'a mut self, index: &'q str) -> &'a mut Properties {
        match self.section_mut(Some(index)) {
            Some(p) => p,
            None => panic!("Section `{}` does not exist", index),
        }
    }
}

impl Ini {
    /// Write to a file
    pub fn write_to_file<P: AsRef<Path>>(&self, filename: P) -> io::Result<()> {
        self.write_to_file_policy(filename, EscapePolicy::Basics)
    }

    /// Write to a file
    pub fn write_to_file_policy<P: AsRef<Path>>(&self, filename: P, policy: EscapePolicy) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(filename.as_ref())?;
        self.write_to_policy(&mut file, policy)
    }

    /// Write to a file with options
    pub fn write_to_file_opt<P: AsRef<Path>>(&self, filename: P, opt: WriteOption) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(filename.as_ref())?;
        self.write_to_opt(&mut file, opt)
    }

    /// Write to a writer
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        self.write_to_opt(writer, Default::default())
    }

    /// Write to a writer
    pub fn write_to_policy<W: Write>(&self, writer: &mut W, policy: EscapePolicy) -> io::Result<()> {
        self.write_to_opt(
            writer,
            WriteOption {
                escape_policy: policy,
                ..Default::default()
            },
        )
    }

    /// Write to a writer with options
    pub fn write_to_opt<W: Write>(&self, writer: &mut W, opt: WriteOption) -> io::Result<()> {
        let mut firstline = true;

        for (section, props) in &self.sections {
            if !props.data.is_empty() {
                if firstline {
                    firstline = false;
                } else {
                    // Write an empty line between sections
                    writer.write_all(opt.line_separator.as_str().as_bytes())?;
                }
            }

            if let Some(ref section) = *section {
                write!(
                    writer,
                    "[{}]{}",
                    escape_str(&section[..], opt.escape_policy),
                    opt.line_separator
                )?;
            }
            for (k, v) in props.iter() {
                let k_str = escape_str(k, opt.escape_policy);
                let v_str = escape_str(v, opt.escape_policy);
                write!(writer, "{}{}{}{}", k_str, opt.kv_separator, v_str, opt.line_separator)?;
            }
        }
        Ok(())
    }
}

impl Ini {
    /// Load from a string
    pub fn load_from_str(buf: &str) -> Result<Ini, ParseError> {
        Ini::load_from_str_opt(buf, ParseOption::default())
    }

    /// Load from a string, but do not interpret '\' as an escape character
    pub fn load_from_str_noescape(buf: &str) -> Result<Ini, ParseError> {
        Ini::load_from_str_opt(
            buf,
            ParseOption {
                enabled_escape: false,
                ..ParseOption::default()
            },
        )
    }

    /// Load from a string with options
    pub fn load_from_str_opt(buf: &str, opt: ParseOption) -> Result<Ini, ParseError> {
        let mut parser = Parser::new(buf.chars(), opt);
        parser.parse()
    }

    /// Load from a reader
    pub fn read_from<R: Read>(reader: &mut R) -> Result<Ini, Error> {
        Ini::read_from_opt(reader, ParseOption::default())
    }

    /// Load from a reader, but do not interpret '\' as an escape character
    pub fn read_from_noescape<R: Read>(reader: &mut R) -> Result<Ini, Error> {
        Ini::read_from_opt(
            reader,
            ParseOption {
                enabled_escape: false,
                ..ParseOption::default()
            },
        )
    }

    /// Load from a reader with options
    pub fn read_from_opt<R: Read>(reader: &mut R, opt: ParseOption) -> Result<Ini, Error> {
        let mut s = String::new();
        reader.read_to_string(&mut s).map_err(Error::Io)?;
        let mut parser = Parser::new(s.chars(), opt);
        match parser.parse() {
            Err(e) => Err(Error::Parse(e)),
            Ok(success) => Ok(success),
        }
    }

    /// Load from a file
    pub fn load_from_file<P: AsRef<Path>>(filename: P) -> Result<Ini, Error> {
        Ini::load_from_file_opt(filename, ParseOption::default())
    }

    /// Load from a file, but do not interpret '\' as an escape character
    pub fn load_from_file_noescape<P: AsRef<Path>>(filename: P) -> Result<Ini, Error> {
        Ini::load_from_file_opt(
            filename,
            ParseOption {
                enabled_escape: false,
                ..ParseOption::default()
            },
        )
    }

    /// Load from a file with options
    pub fn load_from_file_opt<P: AsRef<Path>>(filename: P, opt: ParseOption) -> Result<Ini, Error> {
        let mut reader = match File::open(filename.as_ref()) {
            Err(e) => {
                return Err(Error::Io(e));
            }
            Ok(r) => r,
        };

        let mut with_bom = false;

        // Check if file starts with a BOM marker
        // UTF-8: EF BB BF
        let mut bom = [0u8; 3];
        if reader.read_exact(&mut bom).is_ok() && &bom == b"\xEF\xBB\xBF" {
            with_bom = true;
        }

        if !with_bom {
            // Reset file pointer
            reader.seek(SeekFrom::Start(0))?;
        }

        Ini::read_from_opt(&mut reader, opt)
    }
}

/// Iterator for traversing sections
pub struct SectionIter<'a> {
    inner: Iter<'a, SectionKey, Properties>,
}

impl<'a> Iterator for SectionIter<'a> {
    type Item = (Option<&'a str>, &'a Properties);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (k.as_ref().map(|s| s.as_str()), v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for SectionIter<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|(k, v)| (k.as_ref().map(|s| s.as_str()), v))
    }
}

/// Iterator for traversing sections
pub struct SectionIterMut<'a> {
    inner: IterMut<'a, SectionKey, Properties>,
}

impl<'a> Iterator for SectionIterMut<'a> {
    type Item = (Option<&'a str>, &'a mut Properties);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(k, v)| (k.as_ref().map(|s| s.as_str()), v))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for SectionIterMut<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back().map(|(k, v)| (k.as_ref().map(|s| s.as_str()), v))
    }
}

/// Iterator for traversing sections
pub struct SectionIntoIter {
    inner: IntoIter<SectionKey, Properties>,
}

impl Iterator for SectionIntoIter {
    type Item = (SectionKey, Properties);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl DoubleEndedIterator for SectionIntoIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.next_back()
    }
}

impl<'a> Ini {
    /// Immutable iterate though sections
    pub fn iter(&'a self) -> SectionIter<'a> {
        SectionIter {
            inner: self.sections.iter(),
        }
    }

    /// Mutable iterate though sections
    #[deprecated(note = "Use `iter_mut` instead!")]
    pub fn mut_iter(&'a mut self) -> SectionIterMut<'a> {
        self.iter_mut()
    }

    /// Mutable iterate though sections
    pub fn iter_mut(&'a mut self) -> SectionIterMut<'a> {
        SectionIterMut {
            inner: self.sections.iter_mut(),
        }
    }
}

impl<'a> IntoIterator for &'a Ini {
    type IntoIter = SectionIter<'a>;
    type Item = (Option<&'a str>, &'a Properties);

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut Ini {
    type IntoIter = SectionIterMut<'a>;
    type Item = (Option<&'a str>, &'a mut Properties);

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl IntoIterator for Ini {
    type IntoIter = SectionIntoIter;
    type Item = (SectionKey, Properties);

    fn into_iter(self) -> Self::IntoIter {
        SectionIntoIter {
            inner: self.sections.into_iter(),
        }
    }
}

// Ini parser
struct Parser<'a> {
    ch: Option<char>,
    rdr: Chars<'a>,
    line: usize,
    col: usize,
    opt: ParseOption,
}

#[derive(Debug)]
/// Parse error
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub msg: Cow<'static, str>,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{} {}", self.line, self.col, self.msg)
    }
}

impl error::Error for ParseError {}

/// Error while parsing an INI document
#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Parse(ParseError),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io(ref err) => err.fmt(f),
            Error::Parse(ref err) => err.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match *self {
            Error::Io(ref err) => err.source(),
            Error::Parse(ref err) => err.source(),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl<'a> Parser<'a> {
    // Create a parser
    pub fn new(rdr: Chars<'a>, opt: ParseOption) -> Parser<'a> {
        let mut p = Parser {
            ch: None,
            line: 0,
            col: 0,
            rdr,
            opt,
        };
        p.bump();
        p
    }

    fn bump(&mut self) {
        self.ch = self.rdr.next();
        match self.ch {
            Some('\n') => {
                self.line += 1;
                self.col = 0;
            }
            Some(..) => {
                self.col += 1;
            }
            None => {}
        }
    }

    #[cold]
    #[inline(never)]
    fn error<U, M: Into<Cow<'static, str>>>(&self, msg: M) -> Result<U, ParseError> {
        Err(ParseError {
            line: self.line + 1,
            col: self.col + 1,
            msg: msg.into(),
        })
    }

    #[cold]
    fn eof_error(&self, expecting: &[Option<char>]) -> Result<char, ParseError> {
        self.error(format!("expecting \"{:?}\" but found EOF.", expecting))
    }

    fn char_or_eof(&self, expecting: &[Option<char>]) -> Result<char, ParseError> {
        match self.ch {
            Some(ch) => Ok(ch),
            None => self.eof_error(expecting),
        }
    }

    /// Consume all whitespace including newlines, tabs, and spaces
    ///
    /// This function consumes all types of whitespace characters until it encounters
    /// a non-whitespace character. Used for general whitespace cleanup between tokens.
    // fn parse_whitespace(&mut self) {
    //     while let Some(c) = self.ch {
    //         if !c.is_whitespace() && c != '\n' && c != '\t' && c != '\r' {
    //             break;
    //         }
    //         self.bump();
    //     }
    // }

    /// Consume whitespace but preserve leading spaces/tabs on lines (for key indentation)
    ///
    /// This function is designed to consume whitespace while preserving leading spaces
    /// and tabs that might be part of indented keys. It consumes newlines and other
    /// whitespace, but stops when it encounters spaces or tabs that could be the
    /// beginning of an indented key.
    fn parse_whitespace_preserve_line_leading(&mut self) {
        while let Some(c) = self.ch {
            match c {
                // Always consume spaces and tabs that are not at the beginning of a line
                ' ' | '\t' => {
                    self.bump();
                    // Continue consuming until we hit a non-space/tab character
                    // If it's a comment character, let the caller handle it
                    // If it's a newline, we'll handle it in the next iteration
                }
                '\n' | '\r' => {
                    // Consume the newline
                    self.bump();
                    // Check if the next line starts with spaces/tabs (potential key indentation)
                    if matches!(self.ch, Some(' ') | Some('\t')) {
                        // Don't consume the leading spaces/tabs - they're part of the key
                        break;
                    }
                    // Continue consuming other whitespace after the newline
                }
                c if c.is_whitespace() => {
                    // Consume other whitespace (like form feed, vertical tab, etc.)
                    self.bump();
                }
                _ => break,
            }
        }
    }

    /// Consume all whitespace except line breaks (newlines and carriage returns)
    ///
    /// This function consumes spaces, tabs, and other whitespace characters but
    /// stops at newlines and carriage returns. Used when parsing values to avoid
    /// consuming the line terminator.
    fn parse_whitespace_except_line_break(&mut self) {
        while let Some(c) = self.ch {
            if (c == '\n' || c == '\r' || !c.is_whitespace()) && c != '\t' {
                break;
            }
            self.bump();
        }
    }

    /// Parse the whole INI input
    pub fn parse(&mut self) -> Result<Ini, ParseError> {
        let mut result = Ini::new();
        let mut curkey: String = "".into();
        let mut cursec: Option<String> = None;

        while let Some(cur_ch) = self.ch {
            match cur_ch {
                ';' | '#' => {
                    if cfg!(not(feature = "inline-comment")) {
                        // Inline comments is not supported, so comments must starts from a new line
                        //
                        // https://en.wikipedia.org/wiki/INI_file#Comments
                        if self.col > 1 {
                            return self.error("doesn't support inline comment");
                        }
                    }

                    self.parse_comment();
                }
                '[' => match self.parse_section() {
                    Ok(mut sec) => {
                        trim_in_place(&mut sec);
                        cursec = Some(sec);
                        match result.entry(cursec.clone()) {
                            SectionEntry::Vacant(v) => {
                                v.insert(Default::default());
                            }
                            SectionEntry::Occupied(mut o) => {
                                o.append(Default::default());
                            }
                        }
                    }
                    Err(e) => return Err(e),
                },
                '=' | ':' => {
                    if (curkey[..]).is_empty() {
                        return self.error("missing key");
                    }
                    match self.parse_val() {
                        Ok(mval) => {
                            match result.entry(cursec.clone()) {
                                SectionEntry::Vacant(v) => {
                                    // cursec must be None (the General Section)
                                    let mut prop = Properties::new();
                                    prop.insert(curkey, mval);
                                    v.insert(prop);
                                }
                                SectionEntry::Occupied(mut o) => {
                                    // Insert into the last (current) section
                                    o.last_mut().append(curkey, mval);
                                }
                            }
                            curkey = "".into();
                        }
                        Err(e) => return Err(e),
                    }
                }
                ' ' | '\t' => {
                    // First, consume the leading whitespace to see what comes after
                    let mut consumed_whitespace = String::new();
                    while let Some(c) = self.ch {
                        if c == ' ' || c == '\t' {
                            consumed_whitespace.push(c);
                            self.bump();
                        } else {
                            break;
                        }
                    }

                    // Check if what follows is a section header
                    match self.ch {
                        Some('[') => {
                            // This is a section header, parse it as such
                            match self.parse_section() {
                                Ok(mut sec) => {
                                    trim_in_place(&mut sec);
                                    cursec = Some(sec);
                                    match result.entry(cursec.clone()) {
                                        SectionEntry::Vacant(v) => {
                                            v.insert(Default::default());
                                        }
                                        SectionEntry::Occupied(mut o) => {
                                            o.append(Default::default());
                                        }
                                    }
                                }
                                Err(e) => return Err(e),
                            }
                        }
                        Some('\n') | Some('\r') => {
                            // This is just leading whitespace before a newline, skip it
                            self.bump(); // Consume the newline
                            continue;
                        }
                        _ => {
                            // This is a key with leading whitespace, parse the rest of it
                            match self.parse_str_until(&[Some('='), Some(':')], false) {
                                Ok(key_part) => {
                                    let mut mkey = if self.opt.enabled_preserve_key_leading_whitespace {
                                        consumed_whitespace + &key_part
                                    } else {
                                        key_part
                                    };

                                    // Only trim trailing whitespace, preserve leading whitespace if enabled
                                    if self.opt.enabled_preserve_key_leading_whitespace {
                                        trim_end_in_place(&mut mkey);
                                    } else {
                                        trim_in_place(&mut mkey);
                                    }
                                    curkey = mkey;
                                }
                                Err(_) => {
                                    // If parsing key fails, it's probably just trailing whitespace at EOF - skip it
                                    // We already consumed the whitespace, so just continue
                                }
                            }
                        }
                    }
                }
                '\n' | '\r' => {
                    // Empty line, just skip it
                    self.bump();
                }
                _ => match self.parse_key() {
                    Ok(mut mkey) => {
                        // For regular keys, only trim trailing whitespace to preserve
                        // any leading whitespace that might be part of the key name
                        if self.opt.enabled_preserve_key_leading_whitespace {
                            trim_end_in_place(&mut mkey);
                        } else {
                            trim_in_place(&mut mkey);
                        }
                        curkey = mkey;
                    }
                    Err(e) => return Err(e),
                },
            }

            // Use specialized whitespace parsing that preserves leading spaces/tabs
            // on new lines, which might be part of indented key names
            self.parse_whitespace_preserve_line_leading();
        }

        Ok(result)
    }

    fn parse_comment(&mut self) {
        while let Some(c) = self.ch {
            self.bump();
            if c == '\n' {
                break;
            }
        }
    }

    fn parse_str_until(&mut self, endpoint: &[Option<char>], check_inline_comment: bool) -> Result<String, ParseError> {
        let mut result: String = String::new();

        let mut in_line_continuation = false;

        while !endpoint.contains(&self.ch) {
            match self.char_or_eof(endpoint)? {
                #[cfg(feature = "inline-comment")]
                ch if check_inline_comment && (ch == ' ' || ch == '\t') => {
                    self.bump();

                    match self.ch {
                        Some('#') | Some(';') => {
                            // [space]#, [space]; starts an inline comment
                            self.parse_comment();
                            if in_line_continuation {
                                result.push(ch);
                                continue;
                            } else {
                                break;
                            }
                        }
                        Some(_) => {
                            result.push(ch);
                            continue;
                        }
                        None => {
                            result.push(ch);
                        }
                    }
                }
                #[cfg(feature = "inline-comment")]
                ch if check_inline_comment && in_line_continuation && (ch == '#' || ch == ';') => {
                    self.parse_comment();
                    continue;
                }
                '\\' => {
                    self.bump();
                    let Some(ch) = self.ch else {
                        result.push('\\');
                        continue;
                    };

                    if matches!(ch, '\n') {
                        in_line_continuation = true;
                    } else if self.opt.enabled_escape {
                        match ch {
                            '0' => result.push('\0'),
                            'a' => result.push('\x07'),
                            'b' => result.push('\x08'),
                            't' => result.push('\t'),
                            'r' => result.push('\r'),
                            'n' => result.push('\n'),
                            '\n' => self.bump(),
                            'x' => {
                                // Unicode 4 character
                                let mut code: String = String::with_capacity(4);
                                for _ in 0..4 {
                                    self.bump();
                                    let ch = self.char_or_eof(endpoint)?;
                                    if ch == '\\' {
                                        self.bump();
                                        if self.ch != Some('\n') {
                                            return self.error(format!(
                                                "expecting \"\\\\n\" but \
                                             found \"{:?}\".",
                                                self.ch
                                            ));
                                        }
                                    }

                                    code.push(ch);
                                }
                                let r = u32::from_str_radix(&code[..], 16);
                                match r.ok().and_then(char::from_u32) {
                                    Some(ch) => result.push(ch),
                                    None => return self.error("unknown character in \\xHH form"),
                                }
                            }
                            c => result.push(c),
                        }
                    } else {
                        result.push('\\');
                        result.push(ch);
                    }
                }
                ch => result.push(ch),
            }
            self.bump();
        }

        let _ = check_inline_comment;
        let _ = in_line_continuation;

        Ok(result)
    }

    fn parse_section(&mut self) -> Result<String, ParseError> {
        cfg_if! {
            if #[cfg(feature = "brackets-in-section-names")] {
                // Skip [
                self.bump();

                let mut s = self.parse_str_until(&[Some('\r'), Some('\n')], cfg!(feature = "inline-comment"))?;

                // Deal with inline comment
                #[cfg(feature = "inline-comment")]
                if matches!(self.ch, Some('#') | Some(';')) {
                    self.parse_comment();
                }

                let tr = s.trim_end_matches([' ', '\t']);
                if !tr.ends_with(']') {
                    return self.error("section must be ended with ']'");
                }

                s.truncate(tr.len() - 1);
                Ok(s)
            } else {
                // Skip [
                self.bump();
                let sec = self.parse_str_until(&[Some(']')], false)?;
                if let Some(']') = self.ch {
                    self.bump();
                }

                // Deal with inline comment
                #[cfg(feature = "inline-comment")]
                if matches!(self.ch, Some('#') | Some(';')) {
                    self.parse_comment();
                }

                Ok(sec)
            }
        }
    }

    /// Parse a key name until '=' or ':' delimiter
    ///
    /// This function parses characters until it encounters '=' or ':' which indicate
    /// the start of a value. Used for regular keys without leading whitespace.
    fn parse_key(&mut self) -> Result<String, ParseError> {
        self.parse_str_until(&[Some('='), Some(':')], false)
    }

    fn parse_val(&mut self) -> Result<String, ParseError> {
        self.bump();
        // Issue #35: Allow empty value
        self.parse_whitespace_except_line_break();

        let mut val = String::new();
        let mut val_first_part = true;
        // Parse the first line of value
        'parse_value_line_loop: loop {
            match self.ch {
                // EOF. Just break
                None => break,

                // Double Quoted
                Some('"') if self.opt.enabled_quote => {
                    // Bump the current "
                    self.bump();
                    // Parse until the next "
                    let quoted_val = self.parse_str_until(&[Some('"')], false)?;
                    val.push_str(&quoted_val);

                    // Eats the "
                    self.bump();

                    // characters after " are still part of the value line
                    val_first_part = false;
                    continue;
                }

                // Single Quoted
                Some('\'') if self.opt.enabled_quote => {
                    // Bump the current '
                    self.bump();
                    // Parse until the next '
                    let quoted_val = self.parse_str_until(&[Some('\'')], false)?;
                    val.push_str(&quoted_val);

                    // Eats the '
                    self.bump();

                    // characters after ' are still part of the value line
                    val_first_part = false;
                    continue;
                }

                // Standard value string
                _ => {
                    // Parse until EOL. White spaces are trimmed (both start and end)
                    let standard_val = self.parse_str_until_eol(cfg!(feature = "inline-comment"))?;

                    let trimmed_value = if val_first_part {
                        // If it is the first part of the value, just trim all of them
                        standard_val.trim()
                    } else {
                        // Otherwise, trim the ends
                        standard_val.trim_end()
                    };
                    val_first_part = false;

                    val.push_str(trimmed_value);

                    if self.opt.enabled_indented_mutiline_value {
                        // Multiline value is supported. We now check whether the next line is started with ' ' or '\t'.
                        self.bump();

                        loop {
                            match self.ch {
                                Some(' ') | Some('\t') => {
                                    // Multiline value
                                    // Eats the leading spaces
                                    self.parse_whitespace_except_line_break();
                                    // Push a line-break to the current value
                                    val.push('\n');
                                    // continue. Let read the whole value line
                                    continue 'parse_value_line_loop;
                                }

                                Some('\r') => {
                                    // Probably \r\n, try to eat one more
                                    self.bump();
                                    if self.ch == Some('\n') {
                                        self.bump();
                                        val.push('\n');
                                    } else {
                                        // \r with a character?
                                        return self.error("\\r is not followed by \\n");
                                    }
                                }

                                Some('\n') => {
                                    // New-line, just push and continue
                                    self.bump();
                                    val.push('\n');
                                }

                                // Not part of the multiline value
                                _ => break 'parse_value_line_loop,
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        if self.opt.enabled_indented_mutiline_value {
            // multiline value, trims line-breaks
            trim_line_feeds(&mut val);
        }

        Ok(val)
    }

    #[inline]
    fn parse_str_until_eol(&mut self, check_inline_comment: bool) -> Result<String, ParseError> {
        self.parse_str_until(&[Some('\n'), Some('\r'), None], check_inline_comment)
    }
}

fn trim_in_place(string: &mut String) {
    string.truncate(string.trim_end().len());
    string.drain(..(string.len() - string.trim_start().len()));
}

fn trim_end_in_place(string: &mut String) {
    string.truncate(string.trim_end().len());
}

fn trim_line_feeds(string: &mut String) {
    const LF: char = '\n';
    string.truncate(string.trim_end_matches(LF).len());
    string.drain(..(string.len() - string.trim_start_matches(LF).len()));
}

// ------------------------------------------------------------------------------

#[cfg(test)]
mod test {
    use std::env::temp_dir;

    use super::*;

    #[test]
    fn property_replace() {
        let mut props = Properties::new();
        props.insert("k1", "v1");

        assert_eq!(Some("v1"), props.get("k1"));
        let res = props.get_all("k1").collect::<Vec<&str>>();
        assert_eq!(res, vec!["v1"]);

        props.insert("k1", "v2");
        assert_eq!(Some("v2"), props.get("k1"));

        let res = props.get_all("k1").collect::<Vec<&str>>();
        assert_eq!(res, vec!["v2"]);
    }

    #[test]
    fn property_get_vec() {
        let mut props = Properties::new();
        props.append("k1", "v1");

        assert_eq!(Some("v1"), props.get("k1"));

        props.append("k1", "v2");

        assert_eq!(Some("v1"), props.get("k1"));

        let res = props.get_all("k1").collect::<Vec<&str>>();
        assert_eq!(res, vec!["v1", "v2"]);

        let res = props.get_all("k2").collect::<Vec<&str>>();
        assert!(res.is_empty());
    }

    #[test]
    fn property_remove() {
        let mut props = Properties::new();
        props.append("k1", "v1");
        props.append("k1", "v2");

        let res = props.remove_all("k1").collect::<Vec<String>>();
        assert_eq!(res, vec!["v1", "v2"]);
        assert!(!props.contains_key("k1"));
    }

    #[test]
    fn load_from_str_with_empty_general_section() {
        let input = "[sec1]\nkey1=val1\n";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());

        let mut output = opt.unwrap();
        assert_eq!(output.len(), 2);

        assert!(output.general_section().is_empty());
        assert!(output.general_section_mut().is_empty());

        let props1 = output.section(None::<String>).unwrap();
        assert!(props1.is_empty());
        let props2 = output.section(Some("sec1")).unwrap();
        assert_eq!(props2.len(), 1);
        assert_eq!(props2.get("key1"), Some("val1"));
    }

    #[test]
    fn load_from_str_with_empty_input() {
        let input = "";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());

        let mut output = opt.unwrap();
        assert!(output.general_section().is_empty());
        assert!(output.general_section_mut().is_empty());
        assert_eq!(output.len(), 1);
    }

    #[test]
    fn load_from_str_with_empty_lines() {
        let input = "\n\n\n";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());

        let mut output = opt.unwrap();
        assert!(output.general_section().is_empty());
        assert!(output.general_section_mut().is_empty());
        assert_eq!(output.len(), 1);
    }

    #[test]
    #[cfg(not(feature = "brackets-in-section-names"))]
    fn load_from_str_with_valid_input() {
        let input = "[sec1]\nkey1=val1\nkey2=377\n[sec2]foo=bar\n";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());

        let output = opt.unwrap();
        // there is always a general section
        assert_eq!(output.len(), 3);
        assert!(output.section(Some("sec1")).is_some());

        let sec1 = output.section(Some("sec1")).unwrap();
        assert_eq!(sec1.len(), 2);
        let key1: String = "key1".into();
        assert!(sec1.contains_key(&key1));
        let key2: String = "key2".into();
        assert!(sec1.contains_key(&key2));
        let val1: String = "val1".into();
        assert_eq!(sec1[&key1], val1);
        let val2: String = "377".into();
        assert_eq!(sec1[&key2], val2);
    }

    #[test]
    #[cfg(feature = "brackets-in-section-names")]
    fn load_from_str_with_valid_input() {
        let input = "[sec1]\nkey1=val1\nkey2=377\n[sec2]\nfoo=bar\n";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());

        let output = opt.unwrap();
        // there is always a general section
        assert_eq!(output.len(), 3);
        assert!(output.section(Some("sec1")).is_some());

        let sec1 = output.section(Some("sec1")).unwrap();
        assert_eq!(sec1.len(), 2);
        let key1: String = "key1".into();
        assert!(sec1.contains_key(&key1));
        let key2: String = "key2".into();
        assert!(sec1.contains_key(&key2));
        let val1: String = "val1".into();
        assert_eq!(sec1[&key1], val1);
        let val2: String = "377".into();
        assert_eq!(sec1[&key2], val2);
    }

    #[test]
    #[cfg(not(feature = "brackets-in-section-names"))]
    fn load_from_str_without_ending_newline() {
        let input = "[sec1]\nkey1=val1\nkey2=377\n[sec2]foo=bar";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());
    }

    #[test]
    #[cfg(feature = "brackets-in-section-names")]
    fn load_from_str_without_ending_newline() {
        let input = "[sec1]\nkey1=val1\nkey2=377\n[sec2]\nfoo=bar";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());
    }

    #[test]
    fn parse_error_numbers() {
        let invalid_input = "\n\\x";
        let ini = Ini::load_from_str_opt(
            invalid_input,
            ParseOption {
                enabled_escape: true,
                ..Default::default()
            },
        );
        assert!(ini.is_err());

        let err = ini.unwrap_err();
        assert_eq!(err.line, 2);
        assert_eq!(err.col, 3);
    }

    #[test]
    fn parse_comment() {
        let input = "; abcdefghijklmn\n";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());
    }

    #[cfg(not(feature = "inline-comment"))]
    #[test]
    fn inline_comment_not_supported() {
        let input = "
[section name]
name = hello # abcdefg
gender = mail ; abdddd
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(ini.get_from(Some("section name"), "name").unwrap(), "hello # abcdefg");
        assert_eq!(ini.get_from(Some("section name"), "gender").unwrap(), "mail ; abdddd");
    }

    #[test]
    #[cfg_attr(not(feature = "inline-comment"), should_panic)]
    fn inline_comment() {
        let input = "
[section name] # comment in section line
name = hello # abcdefg
gender = mail ; abdddd
address = web#url ;# eeeeee
phone = 01234	# tab before comment
phone2 = 56789	 # tab + space before comment
phone3 = 43210 	# space + tab before comment
";
        let ini = Ini::load_from_str(input).unwrap();
        println!("{:?}", ini.section(Some("section name")));
        assert_eq!(ini.get_from(Some("section name"), "name").unwrap(), "hello");
        assert_eq!(ini.get_from(Some("section name"), "gender").unwrap(), "mail");
        assert_eq!(ini.get_from(Some("section name"), "address").unwrap(), "web#url");
        assert_eq!(ini.get_from(Some("section name"), "phone").unwrap(), "01234");
        assert_eq!(ini.get_from(Some("section name"), "phone2").unwrap(), "56789");
        assert_eq!(ini.get_from(Some("section name"), "phone3").unwrap(), "43210");
    }

    #[test]
    fn sharp_comment() {
        let input = "
[section name]
name = hello
# abcdefg
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(ini.get_from(Some("section name"), "name").unwrap(), "hello");
    }

    #[test]
    fn iter() {
        let input = "
[section name]
name = hello # abcdefg
gender = mail ; abdddd
";
        let mut ini = Ini::load_from_str(input).unwrap();

        for _ in &mut ini {}
        for _ in &ini {}
        // for _ in ini {}
    }

    #[test]
    fn colon() {
        let input = "
[section name]
name: hello
gender : mail
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(ini.get_from(Some("section name"), "name").unwrap(), "hello");
        assert_eq!(ini.get_from(Some("section name"), "gender").unwrap(), "mail");
    }

    #[test]
    fn string() {
        let input = "
[section name]
# This is a comment
Key = \"Value\"
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(ini.get_from(Some("section name"), "Key").unwrap(), "Value");
    }

    #[test]
    fn string_multiline() {
        let input = "
[section name]
# This is a comment
Key = \"Value
Otherline\"
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(ini.get_from(Some("section name"), "Key").unwrap(), "Value\nOtherline");
    }

    #[test]
    fn string_multiline_escape() {
        let input = r"
[section name]
# This is a comment
Key = Value \
Otherline
";
        let ini = Ini::load_from_str_opt(
            input,
            ParseOption {
                enabled_escape: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ini.get_from(Some("section name"), "Key").unwrap(), "Value Otherline");
    }

    #[cfg(feature = "inline-comment")]
    #[test]
    fn string_multiline_inline_comment() {
        let input = r"
[section name]
# This is a comment
Key = Value \
# This is also a comment
; This is also a comment
   # This is also a comment
Otherline
";
        let ini = Ini::load_from_str_opt(
            input,
            ParseOption {
                enabled_escape: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(ini.get_from(Some("section name"), "Key").unwrap(), "Value    Otherline");
    }

    #[test]
    fn string_comment() {
        let input = "
[section name]
# This is a comment
Key = \"Value   # This is not a comment ; at all\"
Stuff = Other
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(
            ini.get_from(Some("section name"), "Key").unwrap(),
            "Value   # This is not a comment ; at all"
        );
    }

    #[test]
    fn string_single() {
        let input = "
[section name]
# This is a comment
Key = 'Value'
Stuff = Other
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(ini.get_from(Some("section name"), "Key").unwrap(), "Value");
    }

    #[test]
    fn string_includes_quote() {
        let input = "
[Test]
Comment[tr]=İnternet'e erişin
Comment[uk]=Доступ до Інтернету
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(ini.get_from(Some("Test"), "Comment[tr]").unwrap(), "İnternet'e erişin");
    }

    #[test]
    fn string_single_multiline() {
        let input = "
[section name]
# This is a comment
Key = 'Value
Otherline'
Stuff = Other
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(ini.get_from(Some("section name"), "Key").unwrap(), "Value\nOtherline");
    }

    #[test]
    fn string_single_comment() {
        let input = "
[section name]
# This is a comment
Key = 'Value   # This is not a comment ; at all'
";
        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(
            ini.get_from(Some("section name"), "Key").unwrap(),
            "Value   # This is not a comment ; at all"
        );
    }

    #[test]
    fn load_from_str_with_valid_empty_input() {
        let input = "key1=\nkey2=val2\n";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());

        let output = opt.unwrap();
        assert_eq!(output.len(), 1);
        assert!(output.section(None::<String>).is_some());

        let sec1 = output.section(None::<String>).unwrap();
        assert_eq!(sec1.len(), 2);
        let key1: String = "key1".into();
        assert!(sec1.contains_key(&key1));
        let key2: String = "key2".into();
        assert!(sec1.contains_key(&key2));
        let val1: String = "".into();
        assert_eq!(sec1[&key1], val1);
        let val2: String = "val2".into();
        assert_eq!(sec1[&key2], val2);
    }

    #[test]
    fn load_from_str_with_crlf() {
        let input = "key1=val1\r\nkey2=val2\r\n";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());

        let output = opt.unwrap();
        assert_eq!(output.len(), 1);
        assert!(output.section(None::<String>).is_some());
        let sec1 = output.section(None::<String>).unwrap();
        assert_eq!(sec1.len(), 2);
        let key1: String = "key1".into();
        assert!(sec1.contains_key(&key1));
        let key2: String = "key2".into();
        assert!(sec1.contains_key(&key2));
        let val1: String = "val1".into();
        assert_eq!(sec1[&key1], val1);
        let val2: String = "val2".into();
        assert_eq!(sec1[&key2], val2);
    }

    #[test]
    fn load_from_str_with_cr() {
        let input = "key1=val1\rkey2=val2\r";
        let opt = Ini::load_from_str(input);
        assert!(opt.is_ok());

        let output = opt.unwrap();
        assert_eq!(output.len(), 1);
        assert!(output.section(None::<String>).is_some());
        let sec1 = output.section(None::<String>).unwrap();
        assert_eq!(sec1.len(), 2);
        let key1: String = "key1".into();
        assert!(sec1.contains_key(&key1));
        let key2: String = "key2".into();
        assert!(sec1.contains_key(&key2));
        let val1: String = "val1".into();
        assert_eq!(sec1[&key1], val1);
        let val2: String = "val2".into();
        assert_eq!(sec1[&key2], val2);
    }

    #[test]
    #[cfg(not(feature = "brackets-in-section-names"))]
    fn load_from_file_with_bom() {
        let file_name = temp_dir().join("rust_ini_load_from_file_with_bom");

        let file_content = b"\xEF\xBB\xBF[Test]Key=Value\n";

        {
            let mut file = File::create(&file_name).expect("create");
            file.write_all(file_content).expect("write");
        }

        let ini = Ini::load_from_file(&file_name).unwrap();
        assert_eq!(ini.get_from(Some("Test"), "Key"), Some("Value"));
    }

    #[test]
    #[cfg(feature = "brackets-in-section-names")]
    fn load_from_file_with_bom() {
        let file_name = temp_dir().join("rust_ini_load_from_file_with_bom");

        let file_content = b"\xEF\xBB\xBF[Test]\nKey=Value\n";

        {
            let mut file = File::create(&file_name).expect("create");
            file.write_all(file_content).expect("write");
        }

        let ini = Ini::load_from_file(&file_name).unwrap();
        assert_eq!(ini.get_from(Some("Test"), "Key"), Some("Value"));
    }

    #[test]
    #[cfg(not(feature = "brackets-in-section-names"))]
    fn load_from_file_without_bom() {
        let file_name = temp_dir().join("rust_ini_load_from_file_without_bom");

        let file_content = b"[Test]Key=Value\n";

        {
            let mut file = File::create(&file_name).expect("create");
            file.write_all(file_content).expect("write");
        }

        let ini = Ini::load_from_file(&file_name).unwrap();
        assert_eq!(ini.get_from(Some("Test"), "Key"), Some("Value"));
    }

    #[test]
    #[cfg(feature = "brackets-in-section-names")]
    fn load_from_file_without_bom() {
        let file_name = temp_dir().join("rust_ini_load_from_file_without_bom");

        let file_content = b"[Test]\nKey=Value\n";

        {
            let mut file = File::create(&file_name).expect("create");
            file.write_all(file_content).expect("write");
        }

        let ini = Ini::load_from_file(&file_name).unwrap();
        assert_eq!(ini.get_from(Some("Test"), "Key"), Some("Value"));
    }

    #[test]
    fn get_with_non_static_key() {
        let input = "key1=val1\nkey2=val2\n";
        let opt = Ini::load_from_str(input).unwrap();

        let sec1 = opt.section(None::<String>).unwrap();

        let key = "key1".to_owned();
        sec1.get(&key).unwrap();
    }

    #[test]
    fn load_from_str_noescape() {
        let input = "path=C:\\Windows\\Some\\Folder\\";
        let output = Ini::load_from_str_noescape(input).unwrap();
        assert_eq!(output.len(), 1);
        let sec = output.section(None::<String>).unwrap();
        assert_eq!(sec.len(), 1);
        assert!(sec.contains_key("path"));
        assert_eq!(&sec["path"], "C:\\Windows\\Some\\Folder\\");
    }

    #[test]
    fn partial_quoting_double() {
        let input = "
[Section]
A=\"quote\" arg0
B=b";

        let opt = Ini::load_from_str(input).unwrap();
        let sec = opt.section(Some("Section")).unwrap();
        assert_eq!(&sec["A"], "quote arg0");
        assert_eq!(&sec["B"], "b");
    }

    #[test]
    fn partial_quoting_single() {
        let input = "
[Section]
A='quote' arg0
B=b";

        let opt = Ini::load_from_str(input).unwrap();
        let sec = opt.section(Some("Section")).unwrap();
        assert_eq!(&sec["A"], "quote arg0");
        assert_eq!(&sec["B"], "b");
    }

    #[test]
    fn parse_without_quote() {
        let input = "
[Desktop Entry]
Exec = \"/path/to/exe with space\" arg
";

        let opt = Ini::load_from_str_opt(
            input,
            ParseOption {
                enabled_quote: false,
                ..ParseOption::default()
            },
        )
        .unwrap();
        let sec = opt.section(Some("Desktop Entry")).unwrap();
        assert_eq!(&sec["Exec"], "\"/path/to/exe with space\" arg");
    }

    #[test]
    #[cfg(feature = "case-insensitive")]
    fn case_insensitive() {
        let input = "
[SecTION]
KeY=value
";

        let ini = Ini::load_from_str(input).unwrap();
        let section = ini.section(Some("section")).unwrap();
        let val = section.get("key").unwrap();
        assert_eq!("value", val);
    }

    #[test]
    fn preserve_order_section() {
        let input = r"
none2 = n2
[SB]
p2 = 2
[SA]
x2 = 2
[SC]
cd1 = x
[xC]
xd = x
        ";

        let data = Ini::load_from_str(input).unwrap();
        let keys: Vec<Option<&str>> = data.iter().map(|(k, _)| k).collect();

        assert_eq!(keys.len(), 5);
        assert_eq!(keys[0], None);
        assert_eq!(keys[1], Some("SB"));
        assert_eq!(keys[2], Some("SA"));
        assert_eq!(keys[3], Some("SC"));
        assert_eq!(keys[4], Some("xC"));
    }

    #[test]
    fn preserve_order_property() {
        let input = r"
x2 = n2
x1 = n2
x3 = n2
";
        let data = Ini::load_from_str(input).unwrap();
        let section = data.general_section();
        let keys: Vec<&str> = section.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["x2", "x1", "x3"]);
    }

    #[test]
    fn preserve_order_property_in_section() {
        let input = r"
[s]
x2 = n2
xb = n2
a3 = n3
";
        let data = Ini::load_from_str(input).unwrap();
        let section = data.section(Some("s")).unwrap();
        let keys: Vec<&str> = section.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["x2", "xb", "a3"])
    }

    #[test]
    fn preserve_order_write() {
        let input = r"
x2 = n2
x1 = n2
x3 = n2
[s]
x2 = n2
xb = n2
a3 = n3
";
        let data = Ini::load_from_str(input).unwrap();
        let mut buf = vec![];
        data.write_to(&mut buf).unwrap();
        let new_data = Ini::load_from_str(&String::from_utf8(buf).unwrap()).unwrap();

        let sec0 = new_data.general_section();
        let keys0: Vec<&str> = sec0.iter().map(|(k, _)| k).collect();
        assert_eq!(keys0, vec!["x2", "x1", "x3"]);

        let sec1 = new_data.section(Some("s")).unwrap();
        let keys1: Vec<&str> = sec1.iter().map(|(k, _)| k).collect();
        assert_eq!(keys1, vec!["x2", "xb", "a3"]);
    }

    #[test]
    fn write_new() {
        use std::str;

        let ini = Ini::new();

        let opt = WriteOption {
            line_separator: LineSeparator::CR,
            ..Default::default()
        };
        let mut buf = Vec::new();
        ini.write_to_opt(&mut buf, opt).unwrap();

        assert_eq!("", str::from_utf8(&buf).unwrap());
    }

    #[test]
    fn write_line_separator() {
        use std::str;

        let mut ini = Ini::new();
        ini.with_section(Some("Section1"))
            .set("Key1", "Value")
            .set("Key2", "Value");
        ini.with_section(Some("Section2"))
            .set("Key1", "Value")
            .set("Key2", "Value");

        {
            let mut buf = Vec::new();
            ini.write_to_opt(
                &mut buf,
                WriteOption {
                    line_separator: LineSeparator::CR,
                    ..Default::default()
                },
            )
            .unwrap();

            assert_eq!(
                "[Section1]\nKey1=Value\nKey2=Value\n\n[Section2]\nKey1=Value\nKey2=Value\n",
                str::from_utf8(&buf).unwrap()
            );
        }

        {
            let mut buf = Vec::new();
            ini.write_to_opt(
                &mut buf,
                WriteOption {
                    line_separator: LineSeparator::CRLF,
                    ..Default::default()
                },
            )
            .unwrap();

            assert_eq!(
                "[Section1]\r\nKey1=Value\r\nKey2=Value\r\n\r\n[Section2]\r\nKey1=Value\r\nKey2=Value\r\n",
                str::from_utf8(&buf).unwrap()
            );
        }

        {
            let mut buf = Vec::new();
            ini.write_to_opt(
                &mut buf,
                WriteOption {
                    line_separator: LineSeparator::SystemDefault,
                    ..Default::default()
                },
            )
            .unwrap();

            if cfg!(windows) {
                assert_eq!(
                    "[Section1]\r\nKey1=Value\r\nKey2=Value\r\n\r\n[Section2]\r\nKey1=Value\r\nKey2=Value\r\n",
                    str::from_utf8(&buf).unwrap()
                );
            } else {
                assert_eq!(
                    "[Section1]\nKey1=Value\nKey2=Value\n\n[Section2]\nKey1=Value\nKey2=Value\n",
                    str::from_utf8(&buf).unwrap()
                );
            }
        }
    }

    #[test]
    fn write_kv_separator() {
        use std::str;

        let mut ini = Ini::new();
        ini.with_section(None::<String>)
            .set("Key1", "Value")
            .set("Key2", "Value");
        ini.with_section(Some("Section1"))
            .set("Key1", "Value")
            .set("Key2", "Value");
        ini.with_section(Some("Section2"))
            .set("Key1", "Value")
            .set("Key2", "Value");

        let mut buf = Vec::new();
        ini.write_to_opt(
            &mut buf,
            WriteOption {
                kv_separator: " = ",
                ..Default::default()
            },
        )
        .unwrap();

        // Test different line endings in Windows and Unix
        if cfg!(windows) {
            assert_eq!(
                "Key1 = Value\r\nKey2 = Value\r\n\r\n[Section1]\r\nKey1 = Value\r\nKey2 = Value\r\n\r\n[Section2]\r\nKey1 = Value\r\nKey2 = Value\r\n",
                str::from_utf8(&buf).unwrap()
            );
        } else {
            assert_eq!(
                "Key1 = Value\nKey2 = Value\n\n[Section1]\nKey1 = Value\nKey2 = Value\n\n[Section2]\nKey1 = Value\nKey2 = Value\n",
                str::from_utf8(&buf).unwrap()
            );
        }
    }

    #[test]
    fn duplicate_sections() {
        // https://github.com/zonyitoo/rust-ini/issues/49

        let input = r"
[Peer]
foo = a
bar = b

[Peer]
foo = c
bar = d

[Peer]
foo = e
bar = f
";

        let ini = Ini::load_from_str(input).unwrap();
        assert_eq!(3, ini.section_all(Some("Peer")).count());

        let mut iter = ini.iter();
        // there is always an empty general section
        let (k0, p0) = iter.next().unwrap();
        assert_eq!(None, k0);
        assert!(p0.is_empty());
        let (k1, p1) = iter.next().unwrap();
        assert_eq!(Some("Peer"), k1);
        assert_eq!(Some("a"), p1.get("foo"));
        assert_eq!(Some("b"), p1.get("bar"));
        let (k2, p2) = iter.next().unwrap();
        assert_eq!(Some("Peer"), k2);
        assert_eq!(Some("c"), p2.get("foo"));
        assert_eq!(Some("d"), p2.get("bar"));
        let (k3, p3) = iter.next().unwrap();
        assert_eq!(Some("Peer"), k3);
        assert_eq!(Some("e"), p3.get("foo"));
        assert_eq!(Some("f"), p3.get("bar"));

        assert_eq!(None, iter.next());
    }

    #[test]
    fn add_properties_api() {
        // Test duplicate properties in a section
        let mut ini = Ini::new();
        ini.with_section(Some("foo")).add("a", "1").add("a", "2");

        let sec = ini.section(Some("foo")).unwrap();
        assert_eq!(sec.get("a"), Some("1"));
        assert_eq!(sec.get_all("a").collect::<Vec<&str>>(), vec!["1", "2"]);

        // Test add with unique keys
        let mut ini = Ini::new();
        ini.with_section(Some("foo")).add("a", "1").add("b", "2");

        let sec = ini.section(Some("foo")).unwrap();
        assert_eq!(sec.get("a"), Some("1"));
        assert_eq!(sec.get("b"), Some("2"));

        // Test string representation
        let mut ini = Ini::new();
        ini.with_section(Some("foo")).add("a", "1").add("a", "2");
        let mut buf = Vec::new();
        ini.write_to(&mut buf).unwrap();
        let ini_str = String::from_utf8(buf).unwrap();
        if cfg!(windows) {
            assert_eq!(ini_str, "[foo]\r\na=1\r\na=2\r\n");
        } else {
            assert_eq!(ini_str, "[foo]\na=1\na=2\n");
        }
    }

    #[test]
    fn new_has_empty_general_section() {
        let mut ini = Ini::new();

        assert!(ini.general_section().is_empty());
        assert!(ini.general_section_mut().is_empty());
        assert_eq!(ini.len(), 1);
    }

    #[test]
    fn fix_issue63() {
        let section = "PHP";
        let key = "engine";
        let value = "On";
        let new_value = "Off";

        // create a new configuration
        let mut conf = Ini::new();
        conf.with_section(Some(section)).set(key, value);

        // assert the value is the one expected
        let v = conf.get_from(Some(section), key).unwrap();
        assert_eq!(v, value);

        // update the section/key with a new value
        conf.set_to(Some(section), key.to_string(), new_value.to_string());

        // assert the new value was set
        let v = conf.get_from(Some(section), key).unwrap();
        assert_eq!(v, new_value);
    }

    #[test]
    fn fix_issue64() {
        let input = format!("some-key=åäö{}", super::DEFAULT_LINE_SEPARATOR);

        let conf = Ini::load_from_str(&input).unwrap();

        let mut output = Vec::new();
        conf.write_to_policy(&mut output, EscapePolicy::Basics).unwrap();

        assert_eq!(input, String::from_utf8(output).unwrap());
    }

    #[test]
    fn invalid_codepoint() {
        use std::io::Cursor;

        let d = vec![
            10, 8, 68, 8, 61, 10, 126, 126, 61, 49, 10, 62, 8, 8, 61, 10, 91, 93, 93, 36, 91, 61, 10, 75, 91, 10, 10,
            10, 61, 92, 120, 68, 70, 70, 70, 70, 70, 126, 61, 10, 0, 0, 61, 10, 38, 46, 49, 61, 0, 39, 0, 0, 46, 92,
            120, 46, 36, 91, 91, 1, 0, 0, 16, 0, 0, 0, 0, 0, 0,
        ];
        let mut file = Cursor::new(d);
        assert!(Ini::read_from(&mut file).is_err());
    }

    #[test]
    #[cfg(feature = "brackets-in-section-names")]
    fn fix_issue84() {
        let input = "
[[*]]
a = b
c = d
";
        let ini = Ini::load_from_str(input).unwrap();
        let sect = ini.section(Some("[*]"));
        assert!(sect.is_some());
        assert!(sect.unwrap().contains_key("a"));
        assert!(sect.unwrap().contains_key("c"));
    }

    #[test]
    #[cfg(feature = "brackets-in-section-names")]
    fn fix_issue84_brackets_inside() {
        let input = "
[a[b]c]
a = b
c = d
";
        let ini = Ini::load_from_str(input).unwrap();
        let sect = ini.section(Some("a[b]c"));
        assert!(sect.is_some());
        assert!(sect.unwrap().contains_key("a"));
        assert!(sect.unwrap().contains_key("c"));
    }

    #[test]
    #[cfg(feature = "brackets-in-section-names")]
    fn fix_issue84_whitespaces_after_bracket() {
        let input = "
[[*]]\t\t
a = b
c = d
";
        let ini = Ini::load_from_str(input).unwrap();
        let sect = ini.section(Some("[*]"));
        assert!(sect.is_some());
        assert!(sect.unwrap().contains_key("a"));
        assert!(sect.unwrap().contains_key("c"));
    }

    #[test]
    #[cfg(feature = "brackets-in-section-names")]
    fn fix_issue84_not_whitespaces_after_bracket() {
        let input = "
[[*]]xx
a = b
c = d
";
        let ini = Ini::load_from_str(input);
        assert!(ini.is_err());
    }

    #[test]
    fn escape_str_nothing_policy() {
        let test_str = "\0\x07\n字'\"✨🍉杓";
        // This policy should never escape anything.
        let policy = EscapePolicy::Nothing;
        assert_eq!(escape_str(test_str, policy), test_str);
    }

    #[test]
    fn escape_str_basics() {
        let test_backslash = r"\backslashes\";
        let test_nul = "string with \x00nulls\x00 in it";
        let test_controls = "|\x07| bell, |\x08| backspace, |\x7f| delete, |\x1b| escape";
        let test_whitespace = "\t \r\n";

        assert_eq!(escape_str(test_backslash, EscapePolicy::Nothing), test_backslash);
        assert_eq!(escape_str(test_nul, EscapePolicy::Nothing), test_nul);
        assert_eq!(escape_str(test_controls, EscapePolicy::Nothing), test_controls);
        assert_eq!(escape_str(test_whitespace, EscapePolicy::Nothing), test_whitespace);

        for policy in [
            EscapePolicy::Basics,
            EscapePolicy::BasicsUnicode,
            EscapePolicy::BasicsUnicodeExtended,
            EscapePolicy::Reserved,
            EscapePolicy::ReservedUnicode,
            EscapePolicy::ReservedUnicodeExtended,
            EscapePolicy::Everything,
        ] {
            assert_eq!(escape_str(test_backslash, policy), r"\\backslashes\\");
            assert_eq!(escape_str(test_nul, policy), r"string with \0nulls\0 in it");
            assert_eq!(
                escape_str(test_controls, policy),
                r"|\a| bell, |\b| backspace, |\x007f| delete, |\x001b| escape"
            );
            assert_eq!(escape_str(test_whitespace, policy), r"\t \r\n");
        }
    }

    #[test]
    fn escape_str_reserved() {
        // Test reserved characters.
        let test_reserved = ":=;#";
        // And characters which are *not* reserved, but look like they might be.
        let test_punctuation = "!@$%^&*()-_+/?.>,<[]{}``";

        // These policies should *not* escape reserved characters.
        for policy in [
            EscapePolicy::Nothing,
            EscapePolicy::Basics,
            EscapePolicy::BasicsUnicode,
            EscapePolicy::BasicsUnicodeExtended,
        ] {
            assert_eq!(escape_str(test_reserved, policy), ":=;#");
            assert_eq!(escape_str(test_punctuation, policy), test_punctuation);
        }

        // These should.
        for policy in [
            EscapePolicy::Reserved,
            EscapePolicy::ReservedUnicodeExtended,
            EscapePolicy::ReservedUnicode,
            EscapePolicy::Everything,
        ] {
            assert_eq!(escape_str(test_reserved, policy), r"\:\=\;\#");
            assert_eq!(escape_str(test_punctuation, policy), "!@$%^&*()-_+/?.>,<[]{}``");
        }
    }

    #[test]
    fn escape_str_unicode() {
        // Test unicode escapes.
        // The first are Basic Multilingual Plane (BMP) characters - i.e. <= U+FFFF
        // Emoji are above U+FFFF (e.g. in the 1F???? range), and the CJK characters are in the U+20???? range.
        // The last one is for codepoints at the edge of Rust's char type.
        let test_unicode = r"é£∳字✨";
        let test_emoji = r"🐱😉";
        let test_cjk = r"𠈌𠕇";
        let test_high_points = "\u{10ABCD}\u{10FFFF}";

        let policy = EscapePolicy::Nothing;
        assert_eq!(escape_str(test_unicode, policy), test_unicode);
        assert_eq!(escape_str(test_emoji, policy), test_emoji);
        assert_eq!(escape_str(test_high_points, policy), test_high_points);

        // The "Unicode" policies should escape standard BMP unicode, but should *not* escape emoji or supplementary CJK codepoints.
        // The Basics/Reserved policies should behave identically in this regard.
        for policy in [EscapePolicy::BasicsUnicode, EscapePolicy::ReservedUnicode] {
            assert_eq!(escape_str(test_unicode, policy), r"\x00e9\x00a3\x2233\x5b57\x2728");
            assert_eq!(escape_str(test_emoji, policy), test_emoji);
            assert_eq!(escape_str(test_cjk, policy), test_cjk);
            assert_eq!(escape_str(test_high_points, policy), test_high_points);
        }

        // UnicodeExtended policies should escape both BMP and supplementary plane characters.
        for policy in [
            EscapePolicy::BasicsUnicodeExtended,
            EscapePolicy::ReservedUnicodeExtended,
        ] {
            assert_eq!(escape_str(test_unicode, policy), r"\x00e9\x00a3\x2233\x5b57\x2728");
            assert_eq!(escape_str(test_emoji, policy), r"\x1f431\x1f609");
            assert_eq!(escape_str(test_cjk, policy), r"\x2020c\x20547");
            assert_eq!(escape_str(test_high_points, policy), r"\x10abcd\x10ffff");
        }
    }

    #[test]
    fn iter_mut_preserve_order_in_section() {
        let input = r"
x2 = nc
x1 = na
x3 = nb
";
        let mut data = Ini::load_from_str(input).unwrap();
        let section = data.general_section_mut();
        section.iter_mut().enumerate().for_each(|(i, (_, v))| {
            v.push_str(&i.to_string());
        });
        let props: Vec<_> = section.iter().collect();
        assert_eq!(props, vec![("x2", "nc0"), ("x1", "na1"), ("x3", "nb2")]);
    }

    #[test]
    fn preserve_order_properties_into_iter() {
        let input = r"
x2 = nc
x1 = na
x3 = nb
";
        let data = Ini::load_from_str(input).unwrap();
        let (_, section) = data.into_iter().next().unwrap();
        let props: Vec<_> = section.into_iter().collect();
        assert_eq!(
            props,
            vec![
                ("x2".to_owned(), "nc".to_owned()),
                ("x1".to_owned(), "na".to_owned()),
                ("x3".to_owned(), "nb".to_owned())
            ]
        );
    }

    #[test]
    fn section_setter_chain() {
        // fix issue #134

        let mut ini = Ini::new();
        let mut section_setter = ini.with_section(Some("section"));

        // chained set() calls work
        section_setter.set("a", "1").set("b", "2");
        // separate set() calls work
        section_setter.set("c", "3");

        assert_eq!("1", section_setter.get("a").unwrap());
        assert_eq!("2", section_setter.get("b").unwrap());
        assert_eq!("3", section_setter.get("c").unwrap());

        // overwrite values
        section_setter.set("a", "4").set("b", "5");
        section_setter.set("c", "6");

        assert_eq!("4", section_setter.get("a").unwrap());
        assert_eq!("5", section_setter.get("b").unwrap());
        assert_eq!("6", section_setter.get("c").unwrap());

        // delete entries
        section_setter.delete(&"a").delete(&"b");
        section_setter.delete(&"c");

        assert!(section_setter.get("a").is_none());
        assert!(section_setter.get("b").is_none());
        assert!(section_setter.get("c").is_none());
    }

    #[test]
    fn parse_enabled_indented_mutiline_value() {
        let input = "
[Foo]
bar =
    u
    v

baz = w
  x # intentional trailing whitespace below
   y 

 z #2
bla = a
";

        let opt = Ini::load_from_str_opt(
            input,
            ParseOption {
                enabled_indented_mutiline_value: true,
                ..ParseOption::default()
            },
        )
        .unwrap();
        let sec = opt.section(Some("Foo")).unwrap();
        let mut iterator = sec.iter();
        let bar = iterator.next().unwrap().1;
        let baz = iterator.next().unwrap().1;
        let bla = iterator.next().unwrap().1;
        assert!(iterator.next().is_none());
        assert_eq!(bar, "u\nv");
        if cfg!(feature = "inline-comment") {
            assert_eq!(baz, "w\nx\ny\n\nz");
        } else {
            assert_eq!(baz, "w\nx # intentional trailing whitespace below\ny\n\nz #2");
        }
        assert_eq!(bla, "a");
    }

    #[test]
    fn whitespace_inside_quoted_value_should_not_be_trimed() {
        let input = r#"
[Foo]
Key=   "  quoted with whitespace "  
        "#;

        let opt = Ini::load_from_str_opt(
            input,
            ParseOption {
                enabled_quote: true,
                ..ParseOption::default()
            },
        )
        .unwrap();

        assert_eq!("  quoted with whitespace ", opt.get_from(Some("Foo"), "Key").unwrap());
    }

    #[test]
    fn preserve_leading_whitespace_in_keys() {
        // Test this particular case in AWS Config files
        // https://docs.aws.amazon.com/cli/v1/userguide/cli-configure-files.html#cli-config-endpoint_url
        let input = r"[profile dev]
services=my-services

[services my-services]
dynamodb=
  endpoint_url=http://localhost:8000
";

        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;

        let data = Ini::load_from_str_opt(input, opts).unwrap();
        let mut w = Vec::new();
        data.write_to(&mut w).ok();
        let output = String::from_utf8(w).ok().unwrap();

        // Normalize line endings for cross-platform compatibility
        let normalized_input = input.replace('\r', "");
        let normalized_output = output.replace('\r', "");
        assert_eq!(normalized_input, normalized_output);
    }

    #[test]
    fn preserve_leading_whitespace_mixed_indentation() {
        let input = r"[section]
key1=value1
  key2=value2
    key3=value3
";
        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;

        let data = Ini::load_from_str_opt(input, opts).unwrap();
        let section = data.section(Some("section")).unwrap();

        // Check that leading whitespace is preserved
        assert!(section.contains_key("key1"));
        assert!(section.contains_key("  key2"));
        assert!(section.contains_key("    key3"));

        // Check round-trip preservation with normalized line endings
        let mut w = Vec::new();
        data.write_to(&mut w).ok();
        let output = String::from_utf8(w).ok().unwrap();
        let normalized_input = input.replace('\r', "");
        let normalized_output = output.replace('\r', "");
        assert_eq!(normalized_input, normalized_output);
    }

    #[test]
    fn preserve_leading_whitespace_tabs_get_escaped() {
        // This test documents the current behavior: tabs in keys get escaped
        let input = r"[section]
	key1=value1
";
        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;

        let data = Ini::load_from_str_opt(input, opts).unwrap();
        let section = data.section(Some("section")).unwrap();

        // The tab is preserved during parsing
        assert!(section.contains_key("\tkey1"));
        assert_eq!(section.get("\tkey1"), Some("value1"));

        // But tabs get escaped during writing (this is expected INI behavior)
        let mut w = Vec::new();
        data.write_to(&mut w).ok();
        let output = String::from_utf8(w).ok().unwrap();

        // Normalize line endings and check that tab is escaped
        let normalized_output = output.replace('\r', "");
        let expected = "[section]\n\\tkey1=value1\n";
        assert_eq!(normalized_output, expected);
    }

    #[test]
    fn preserve_leading_whitespace_with_trailing_spaces() {
        let input = r"[section]
  key1  =value1
    key2	=value2
";
        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;

        let data = Ini::load_from_str_opt(input, opts).unwrap();
        let section = data.section(Some("section")).unwrap();

        // Leading whitespace should be preserved, trailing whitespace in keys should be trimmed
        assert!(section.contains_key("  key1"));
        assert!(section.contains_key("    key2"));
        assert_eq!(section.get("  key1"), Some("value1"));
        assert_eq!(section.get("    key2"), Some("value2"));
    }

    #[test]
    fn section_after_whitespace_bug_reproduction() {
        let input = "[SectionA]\nKey1=Value1\n\n  [SectionB]\n  Key2=Value2";

        // Test with default options (whitespace preservation disabled)
        let data_default = Ini::load_from_str(input).unwrap();

        // Should have two sections
        assert!(data_default.section(Some("SectionA")).is_some());
        assert!(data_default.section(Some("SectionB")).is_some());

        let section_a = data_default.section(Some("SectionA")).unwrap();
        let section_b = data_default.section(Some("SectionB")).unwrap();

        assert_eq!(section_a.get("Key1"), Some("Value1"));
        assert_eq!(section_b.get("Key2"), Some("Value2"));

        // Test with whitespace preservation enabled
        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;
        let data_preserve = Ini::load_from_str_opt(input, opts).unwrap();

        // Should still have two sections
        assert!(data_preserve.section(Some("SectionA")).is_some());
        assert!(data_preserve.section(Some("SectionB")).is_some());

        let section_a_preserve = data_preserve.section(Some("SectionA")).unwrap();
        let section_b_preserve = data_preserve.section(Some("SectionB")).unwrap();

        assert_eq!(section_a_preserve.get("Key1"), Some("Value1"));
        // With whitespace preservation, the key includes leading whitespace
        assert_eq!(section_b_preserve.get("  Key2"), Some("Value2"));
    }

    #[test]
    fn section_after_tabs_and_spaces() {
        // Test with mixed tabs and spaces before section
        let input = "[SectionA]\nKey1=Value1\n\n\t  [SectionB]\n\t  Key2=Value2";

        let data_default = Ini::load_from_str(input).unwrap();

        assert!(data_default.section(Some("SectionA")).is_some());
        assert!(data_default.section(Some("SectionB")).is_some());

        let section_a = data_default.section(Some("SectionA")).unwrap();
        let section_b = data_default.section(Some("SectionB")).unwrap();

        assert_eq!(section_a.get("Key1"), Some("Value1"));
        assert_eq!(section_b.get("Key2"), Some("Value2"));
    }

    #[test]
    fn multiple_sections_with_whitespace() {
        // Test multiple sections with whitespace
        let input = "[SectionA]\nKey1=Value1\n\n  [SectionB]\n  Key2=Value2\n\n    [SectionC]\n    Key3=Value3";

        let data = Ini::load_from_str(input).unwrap();

        assert!(data.section(Some("SectionA")).is_some());
        assert!(data.section(Some("SectionB")).is_some());
        assert!(data.section(Some("SectionC")).is_some());

        assert_eq!(data.section(Some("SectionA")).unwrap().get("Key1"), Some("Value1"));
        assert_eq!(data.section(Some("SectionB")).unwrap().get("Key2"), Some("Value2"));
        assert_eq!(data.section(Some("SectionC")).unwrap().get("Key3"), Some("Value3"));
    }

    #[test]
    fn section_after_whitespace_bug_reproduction_preserve_enabled() {
        let input = "[SectionA]\nKey1=Value1\n\n  [SectionB]\n  Key2=Value2";

        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;
        let data = Ini::load_from_str_opt(input, opts).unwrap();

        assert!(data.section(Some("SectionA")).is_some());
        assert!(data.section(Some("SectionB")).is_some());

        let section_a = data.section(Some("SectionA")).unwrap();
        let section_b = data.section(Some("SectionB")).unwrap();

        assert_eq!(section_a.get("Key1"), Some("Value1"));
        assert_eq!(section_b.get("  Key2"), Some("Value2"));
    }

    #[test]
    fn section_after_tabs_and_spaces_preserve_enabled() {
        let input = "[SectionA]\nKey1=Value1\n\n\t  [SectionB]\n\t  Key2=Value2";

        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;
        let data = Ini::load_from_str_opt(input, opts).unwrap();

        assert!(data.section(Some("SectionA")).is_some());
        assert!(data.section(Some("SectionB")).is_some());

        let section_a = data.section(Some("SectionA")).unwrap();
        let section_b = data.section(Some("SectionB")).unwrap();

        assert_eq!(section_a.get("Key1"), Some("Value1"));
        assert_eq!(section_b.get("\t  Key2"), Some("Value2"));
    }

    #[test]
    fn multiple_sections_with_whitespace_preserve_enabled() {
        let input = "[SectionA]\nKey1=Value1\n\n  [SectionB]\n  Key2=Value2\n\n    [SectionC]\n    Key3=Value3";

        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;
        let data = Ini::load_from_str_opt(input, opts).unwrap();

        assert!(data.section(Some("SectionA")).is_some());
        assert!(data.section(Some("SectionB")).is_some());
        assert!(data.section(Some("SectionC")).is_some());

        assert_eq!(data.section(Some("SectionA")).unwrap().get("Key1"), Some("Value1"));
        assert_eq!(data.section(Some("SectionB")).unwrap().get("  Key2"), Some("Value2"));
        assert_eq!(data.section(Some("SectionC")).unwrap().get("    Key3"), Some("Value3"));
    }

    #[test]
    fn general_section_with_key_leading_whitespace() {
        let input = "\n\n\n\r\n  key1=value1\n\tkey2=value2\nkey3=value3\n[Section]\nkeyA=valueA";
        let mut opts = ParseOption::default();
        opts.enabled_preserve_key_leading_whitespace = true;
        let data = Ini::load_from_str_opt(input, opts).unwrap();
        let general = data.general_section();
        assert_eq!(general.get("  key1"), Some("value1"));
        assert_eq!(general.get("\tkey2"), Some("value2"));
        assert_eq!(general.get("key3"), Some("value3"));
        let section = data.section(Some("Section")).unwrap();
        assert_eq!(section.get("keyA"), Some("valueA"));
    }
}
