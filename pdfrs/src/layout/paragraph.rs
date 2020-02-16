use std::iter;

use super::style::Style;
use unicode_linebreak::{linebreaks_iter, BreakOpportunity};

/// A type that contains a text paragraph, which consists of chunks of styled text.
pub struct Paragraph<'a> {
    pub children: Vec<TextNode<'a>>,
}

/// A styled text node used as a building-block for paragraphs.
#[derive(Clone)]
pub struct TextNode<'a> {
    pub text: &'a str,
    pub style: &'a Style<'a>,
}

/// A chunk of text that is styled and optionally has a possible trailing line-break.
#[derive(Debug, PartialEq)]
struct TextChunk<'a> {
    text: &'a str,
    style: &'a Style<'a>,
    break_after: Option<BreakOpportunity>,
}

impl<'a> Paragraph<'a> {
    /// Splits the paragraph into text nodes by possible line-breaks.
    fn chunks(&'a self) -> impl Iterator<Item = TextChunk<'a>> {
        let mut linebreaks = linebreaks_iter(self.children.iter().map(|node| node.text));
        let mut next_break = linebreaks.next();

        let mut nodes = self.children.iter();
        let mut next_node = nodes.next().cloned();

        // the offset is used to derive the current text position across all text nodes
        let mut offset = 0;

        iter::from_fn(move || {
            if let (Some(mut node), Some((i, br))) = (
                next_node.take().or_else(|| nodes.next().cloned()),
                next_break.take().or_else(|| linebreaks.next()),
            ) {
                // calculate the break position relative to the current text node
                let pos = i - offset;

                if pos > node.text.len() {
                    // keep the possible line-break for the next node (next iteration)
                    next_break = Some((i, br));
                    offset += node.text.len();

                    // return the remaining text of the current node if the next possible line-break
                    // is not within the current node
                    Some(TextChunk {
                        text: node.text,
                        style: &node.style,
                        break_after: None,
                    })
                } else {
                    // split the current node at the possible line-break and return the
                    // corresponding chunk

                    let (word, remaining) = node.text.split_at(pos);
                    let chunk = TextChunk {
                        text: word,
                        style: &node.style,
                        break_after: Some(br),
                    };

                    // if there is still text left for the current node, keep it for the next
                    // iteration
                    if !remaining.is_empty() {
                        node.text = remaining;
                        next_node = Some(node);
                    }
                    offset = i;

                    Some(chunk)
                }
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn paragraph_chunks_optional_break() {
        let style = Style {
            font: &*crate::fonts::HELVETICA,
        };

        let p = Paragraph {
            children: vec![TextNode {
                text: "foo-bar",
                style: &style,
            }],
        };

        assert_eq!(
            p.chunks().collect::<Vec<_>>(),
            vec![
                TextChunk {
                    text: "foo-",
                    style: &style,
                    break_after: Some(BreakOpportunity::Allowed)
                },
                TextChunk {
                    text: "bar",
                    style: &style,
                    break_after: Some(BreakOpportunity::Mandatory)
                },
            ]
        );
    }

    #[test]
    fn paragraph_chunks_mandatory_break() {
        let style = Style {
            font: &*crate::fonts::HELVETICA,
        };

        let p = Paragraph {
            children: vec![TextNode {
                text: "foo\nbar",
                style: &style,
            }],
        };

        assert_eq!(
            p.chunks().collect::<Vec<_>>(),
            vec![
                TextChunk {
                    text: "foo\n",
                    style: &style,
                    break_after: Some(BreakOpportunity::Mandatory)
                },
                TextChunk {
                    text: "bar",
                    style: &style,
                    break_after: Some(BreakOpportunity::Mandatory)
                },
            ]
        );
    }

    #[test]
    fn paragraph_chunks_multiple_text_nodes() {
        let style = Style {
            font: &*crate::fonts::HELVETICA,
        };

        let p = Paragraph {
            children: vec![
                TextNode {
                    text: "This ",
                    style: &style,
                },
                TextNode {
                    text: "works.",
                    style: &style,
                },
            ],
        };

        assert_eq!(
            p.chunks().collect::<Vec<_>>(),
            vec![
                TextChunk {
                    text: "This ",
                    style: &style,
                    break_after: Some(BreakOpportunity::Allowed)
                },
                TextChunk {
                    text: "works.",
                    style: &style,
                    break_after: Some(BreakOpportunity::Mandatory)
                }
            ]
        );
    }

    #[test]
    fn paragraph_chunks_two_nodes_without_breaks() {
        let style = Style {
            font: &*crate::fonts::HELVETICA,
        };

        let p = Paragraph {
            children: vec![
                TextNode {
                    text: "foo",
                    style: &style,
                },
                TextNode {
                    text: "bar",
                    style: &style,
                },
            ],
        };

        assert_eq!(
            p.chunks().collect::<Vec<_>>(),
            vec![
                TextChunk {
                    text: "foo",
                    style: &style,
                    break_after: None
                },
                TextChunk {
                    text: "bar",
                    style: &style,
                    break_after: Some(BreakOpportunity::Mandatory)
                }
            ]
        );
    }

    #[test]
    fn paragraph_chunks_three_nodes_without_breaks() {
        let style = Style {
            font: &*crate::fonts::HELVETICA,
        };

        let p = Paragraph {
            children: vec![
                TextNode {
                    text: "fo",
                    style: &style,
                },
                TextNode {
                    text: "ob",
                    style: &style,
                },
                TextNode {
                    text: "ar is!",
                    style: &style,
                },
            ],
        };

        assert_eq!(
            p.chunks().collect::<Vec<_>>(),
            vec![
                TextChunk {
                    text: "fo",
                    style: &style,
                    break_after: None
                },
                TextChunk {
                    text: "ob",
                    style: &style,
                    break_after: None
                },
                TextChunk {
                    text: "ar ",
                    style: &style,
                    break_after: Some(BreakOpportunity::Allowed)
                },
                TextChunk {
                    text: "is!",
                    style: &style,
                    break_after: Some(BreakOpportunity::Mandatory)
                }
            ]
        );
    }
}
