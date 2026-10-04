use super::description_model::{DescriptionSpan, DescriptionStream};
use super::description_stream::{encode_description_stream, parse_description_stream};

#[test]
fn parses_multiple_spans_and_lines_without_treating_width_as_a_token() {
    let bytes = [0x00, 0x0a, 0x0c, 0x02, 0xfe, 0x18, 0x07, 0xff];

    let stream = parse_description_stream(&bytes).unwrap();

    assert_eq!(stream.lines.len(), 2);
    assert_eq!(
        stream.lines[0],
        [
            DescriptionSpan {
                start_cell: 0,
                cell_count: 10,
            },
            DescriptionSpan {
                start_cell: 12,
                cell_count: 2,
            },
        ]
    );
    assert_eq!(encode_description_stream(&stream).unwrap(), bytes);
}

#[test]
fn rejects_a_span_that_crosses_the_twelve_cell_texture_row() {
    let error = parse_description_stream(&[0x0b, 0x02, 0xff]).unwrap_err();

    assert!(error.to_string().contains("crosses an atlas row"));
}

#[test]
fn rejects_empty_lines_instead_of_silently_dropping_them() {
    let error = parse_description_stream(&[0x00, 0x01, 0xfe, 0xff]).unwrap_err();

    assert!(error.to_string().contains("empty line"));
}

#[test]
fn encoder_rejects_control_values_as_span_starts() {
    let stream = DescriptionStream {
        lines: vec![vec![DescriptionSpan {
            start_cell: 0xfe,
            cell_count: 1,
        }]],
    };

    let error = encode_description_stream(&stream).unwrap_err();

    assert!(error.to_string().contains("control byte"));
}
