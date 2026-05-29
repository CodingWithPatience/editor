use crate::annotation_type::AnnotationType;

#[derive(Debug)]
pub struct AnnotatedStringPart<'a> {
    pub string: &'a str,
    pub annotated_type: Option<AnnotationType>,
}
