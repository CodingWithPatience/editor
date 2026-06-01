use crate::annotated_string::annotated_string_iterator::AnnotatedStringIterator;
use crate::annotated_string::annotated_string_part::AnnotatedStringPart;
use crate::annotation::Annotation;
use crate::annotation_type::AnnotationType;
use crate::prelude::ByteIdx;
use std::cmp::{max, min};
use std::fmt::{Display, Formatter};

mod annotated_string_iterator;
mod annotated_string_part;

#[derive(Default, Debug)]
pub struct AnnotatedString {
    string: String,
    annotations: Vec<Annotation>,
}

impl AnnotatedString {
    pub fn from(string: &str) -> Self {
        Self {
            string: String::from(string),
            annotations: Vec::new(),
        }
    }

    pub fn byte_len(&self) -> usize {
        self.string.len()
    }

    pub fn add_annotation(&mut self, annotation_type: AnnotationType, start: usize, end: usize) {
        debug_assert!(start <= end);
        self.annotations.push(Annotation {
            annotation_type,
            start,
            end,
        })
    }

    pub fn truncate_left_until(&mut self, until: ByteIdx) {
        self.replace(0, until, "");
    }

    pub fn truncate_right_from(&mut self, from: ByteIdx) {
        self.replace(from, self.string.len(), "");
    }

    pub fn replace(&mut self, start: usize, end: usize, new_string: &str) {
        let end = min(end, self.string.len());
        debug_assert!(start <= end);
        if start > end {
            return;
        }
        self.string.replace_range(start..end, new_string);
        let replace_range_len = end.saturating_sub(start);
        let shortened = new_string.len() < replace_range_len;
        let len_difference = new_string.len().abs_diff(replace_range_len);
        if len_difference == 0 {
            return;
        }

        self.annotations.iter_mut().for_each(|annotation| {
            annotation.start = if annotation.start > end {
                if shortened {
                    annotation.start.saturating_sub(len_difference)
                } else {
                    annotation.start.saturating_add(len_difference)
                }
            } else if annotation.start >= start {
                if shortened {
                    max(start, annotation.start.saturating_sub(len_difference))
                } else {
                    min(end, annotation.start.saturating_add(len_difference))
                }
            } else {
                annotation.start
            };

            annotation.end = if annotation.end > end {
                if shortened {
                    annotation.end.saturating_sub(len_difference)
                } else {
                    annotation.end.saturating_add(len_difference)
                }
            } else if annotation.end >= start {
                if shortened {
                    max(start, annotation.end.saturating_sub(len_difference))
                } else {
                    min(end, annotation.end.saturating_add(len_difference))
                }
            } else {
                annotation.end
            };
        });

        self.annotations.retain(|annotation| {
            annotation.start < annotation.end && annotation.start < self.string.len()
        });
    }
}

impl Display for AnnotatedString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.string)
    }
}

impl<'a> IntoIterator for &'a AnnotatedString {
    type Item = AnnotatedStringPart<'a>;
    type IntoIter = AnnotatedStringIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        AnnotatedStringIterator {
            annotated_string: self,
            current_idx: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_string() {
        let s = AnnotatedString::from("hello");
        assert_eq!(s.to_string(), "hello");
        assert_eq!(s.byte_len(), 5);
    }

    #[test]
    fn from_empty() {
        let s = AnnotatedString::from("");
        assert_eq!(s.to_string(), "");
        assert_eq!(s.byte_len(), 0);
    }

    #[test]
    fn add_annotation() {
        let mut s = AnnotatedString::from("hello world");
        s.add_annotation(AnnotationType::Keyword, 0, 5);
        let parts: Vec<_> = (&s).into_iter().collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].string, "hello");
        assert_eq!(parts[0].annotated_type, Some(AnnotationType::Keyword));
        assert_eq!(parts[1].string, " world");
        assert_eq!(parts[1].annotated_type, None);
    }

    #[test]
    fn add_multiple_annotations() {
        let mut s = AnnotatedString::from("hello world");
        s.add_annotation(AnnotationType::Keyword, 0, 5);
        s.add_annotation(AnnotationType::String, 6, 11);
        let parts: Vec<_> = (&s).into_iter().collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].annotated_type, Some(AnnotationType::Keyword));
        assert_eq!(parts[1].annotated_type, None); // space
        assert_eq!(parts[2].annotated_type, Some(AnnotationType::String));
    }

    #[test]
    fn truncate_left() {
        let mut s = AnnotatedString::from("hello world");
        s.add_annotation(AnnotationType::Keyword, 0, 5);
        s.truncate_left_until(6);
        assert_eq!(s.to_string(), "world");
    }

    #[test]
    fn truncate_right() {
        let mut s = AnnotatedString::from("hello world");
        s.truncate_right_from(5);
        assert_eq!(s.to_string(), "hello");
    }

    #[test]
    fn replace_middle() {
        let mut s = AnnotatedString::from("hello world");
        s.replace(5, 6, "-");
        assert_eq!(s.to_string(), "hello-world");
    }

    #[test]
    fn replace_with_annotation_adjust() {
        let mut s = AnnotatedString::from("hello world");
        s.add_annotation(AnnotationType::String, 6, 11);
        s.replace(0, 5, "hi");
        // "hi world" — annotation should adjust
        assert_eq!(s.to_string(), "hi world");
        let parts: Vec<_> = (&s).into_iter().collect();
        let string_part = parts.iter().find(|p| p.annotated_type == Some(AnnotationType::String));
        assert!(string_part.is_some());
        assert_eq!(string_part.unwrap().string, "world");
    }

    #[test]
    fn iterator_no_annotations() {
        let s = AnnotatedString::from("hello");
        let parts: Vec<_> = (&s).into_iter().collect();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].string, "hello");
        assert_eq!(parts[0].annotated_type, None);
    }

    #[test]
    fn display() {
        let s = AnnotatedString::from("test");
        assert_eq!(format!("{s}"), "test");
    }
}
