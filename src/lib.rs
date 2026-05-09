#![warn(missing_docs)]
#![warn(clippy::std_instead_of_core)]
#![warn(clippy::std_instead_of_alloc)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate self as facet_jsonschema;

use facet::{
    Def, Facet, PointerDef, PointerType, PrimitiveType, Shape, TextualType, Type, UserType,
};

use core::alloc::Layout;
use std::io::Write;

facet::define_attr_grammar! {
    ns "facet_jsonschema";
    crate_path ::facet_jsonschema;

    /// JSON Schema-specific Facet attributes.
    pub enum Attr {
        /// Sets the top-level JSON Schema `$id`.
        Id(&'static str),
    }
}

/// Convert a `Facet` type to a JSON schema string.
pub fn to_string<'a, T: Facet<'a>>() -> String {
    let mut buffer = Vec::new();
    write!(buffer, "{{").unwrap();
    write!(
        buffer,
        "\"$schema\": \"https://json-schema.org/draft/2020-12/schema\","
    )
    .unwrap();

    // JSON Schema only allows a single top-level `$id`.
    let mut id = T::SHAPE.attributes.iter().filter_map(|attr| {
        (attr.ns == Some("facet_jsonschema") && attr.key == "id")
            .then(|| attr.get_as::<&'static str>())
            .flatten()
            .copied()
    });
    match (id.next(), id.next()) {
        (Some(_), Some(_)) => panic!("More than one id attribute found"),
        (Some(id), None) => {
            write!(buffer, "\"$id\": \"{id}\",").unwrap();
        }
        _ => {
            // No id attribute found, do nothing
        }
    }

    serialize(T::SHAPE, &[], &mut buffer).unwrap();
    write!(buffer, "}}").unwrap();
    String::from_utf8(buffer).unwrap()
}

fn serialize<W: Write>(shape: &Shape, doc: &[&str], writer: &mut W) -> std::io::Result<()> {
    serialize_doc(&[shape.doc, doc].concat(), writer)?;

    // First check the type system (Type)
    match &shape.ty {
        Type::User(UserType::Struct(struct_def)) => {
            serialize_struct(struct_def, writer)?;
            return Ok(());
        }
        Type::User(UserType::Enum(_enum_def)) => {
            todo!("Enum");
        }
        Type::Sequence(sequence_type) => {
            use facet::SequenceType;
            match sequence_type {
                SequenceType::Slice(_slice_type) => {
                    // For slices, use the Def::Slice if available
                    if let Def::Slice(slice_def) = shape.def {
                        serialize_slice(slice_def, writer)?;
                        return Ok(());
                    }
                }
                SequenceType::Array(_array_type) => {
                    // For arrays, use the Def::Array if available
                    if let Def::Array(array_def) = shape.def {
                        serialize_array(array_def, writer)?;
                        return Ok(());
                    }
                }
            }
        }
        Type::Pointer(PointerType::Reference(pt) | PointerType::Raw(pt)) => {
            serialize(pt.target(), &[], writer)?;
            return Ok(());
        }
        _ => {} // Continue to check the def system
    }

    // Then check the def system (Def)
    match shape.def {
        Def::Scalar => match shape.ty {
            Type::Primitive(PrimitiveType::Numeric(numeric_type)) => {
                serialize_scalar(&shape.layout.sized_layout().unwrap(), numeric_type, writer)?
            }
            Type::Primitive(PrimitiveType::Boolean) => {
                write!(writer, "\"type\": \"boolean\"")?;
            }
            Type::Primitive(PrimitiveType::Textual(TextualType::Str)) => {
                write!(writer, "\"type\": \"string\"")?;
            }
            Type::Primitive(PrimitiveType::Textual(TextualType::Char)) => {
                write!(writer, "\"type\": \"string\", \"maxLength\": 1")?;
            }
            _ => {
                // For other scalar types (like Path, UUID, etc.), default to string
                write!(writer, "\"type\": \"string\"")?;
            }
        },
        Def::Map(_map_def) => todo!("Map"),
        Def::List(list_def) => serialize_list(list_def, writer)?,
        Def::Slice(slice_def) => serialize_slice(slice_def, writer)?,
        Def::Array(array_def) => serialize_array(array_def, writer)?,
        Def::Option(option_def) => serialize_option(option_def, writer)?,
        Def::Pointer(PointerDef {
            pointee: Some(inner_shape),
            ..
        }) => serialize(inner_shape, &[], writer)?,
        Def::Undefined => {
            // Handle the case when not yet migrated to the Type enum
            // For primitives, we can try to infer the type
            match &shape.ty {
                Type::Primitive(primitive) => {
                    use facet::{NumericType, PrimitiveType, TextualType};
                    match primitive {
                        PrimitiveType::Numeric(NumericType::Float) => {
                            write!(writer, "\"type\": \"number\", \"format\": \"double\"")?;
                        }
                        PrimitiveType::Boolean => {
                            write!(writer, "\"type\": \"boolean\"")?;
                        }
                        PrimitiveType::Textual(TextualType::Str) => {
                            write!(writer, "\"type\": \"string\"")?;
                        }
                        _ => {
                            write!(writer, "\"type\": \"unknown\"")?;
                        }
                    }
                }
                Type::Pointer(PointerType::Reference(pt) | PointerType::Raw(pt)) => {
                    serialize(pt.target(), &[], writer)?
                }
                _ => {
                    write!(writer, "\"type\": \"unknown\"")?;
                }
            }
        }
        _ => {
            write!(writer, "\"type\": \"unknown\"")?;
        }
    }

    Ok(())
}

fn serialize_doc<W: Write>(doc: &[&str], writer: &mut W) -> Result<(), std::io::Error> {
    if !doc.is_empty() {
        let doc = doc.join("\n");
        write!(writer, "\"description\": \"{}\",", doc.trim())?;
    }
    Ok(())
}

/// Serialize a scalar definition to JSON schema format.
fn serialize_scalar<W: Write>(
    layout: &Layout,
    numeric_type: facet::NumericType,
    writer: &mut W,
) -> std::io::Result<()> {
    use facet::NumericType;

    match numeric_type {
        NumericType::Integer { signed } => {
            write!(writer, "\"type\": \"integer\"")?;
            let bits = layout.size() * 8;
            if signed {
                write!(writer, ", \"format\": \"int{bits}\"")?;
            } else {
                write!(writer, ", \"format\": \"uint{bits}\"")?;
                write!(writer, ", \"minimum\": 0")?;
            }
        }
        NumericType::Float => {
            write!(writer, "\"type\": \"number\"")?;
            write!(writer, ", \"format\": \"double\"")?;
        }
    }
    Ok(())
}

fn serialize_struct<W: Write>(
    struct_type: &facet::StructType,
    writer: &mut W,
) -> std::io::Result<()> {
    write!(writer, "\"type\": \"object\",")?;
    let required = struct_type
        .fields
        .iter()
        .map(|f| format!("\"{}\"", f.name))
        .collect::<Vec<_>>()
        .join(",");
    write!(writer, "\"required\": [{required}],")?;
    write!(writer, "\"properties\": {{")?;
    let mut first = true;
    for field in struct_type.fields {
        if !first {
            write!(writer, ",")?;
        }
        first = false;
        write!(writer, "\"{}\": {{", field.name)?;
        serialize(field.shape(), field.doc, writer)?;
        write!(writer, "}}")?;
    }
    write!(writer, "}}")?;
    Ok(())
}

/// Serialize a list definition to JSON schema format.
fn serialize_list<W: Write>(list_def: facet::ListDef, writer: &mut W) -> std::io::Result<()> {
    write!(writer, "\"type\": \"array\",")?;
    write!(writer, "\"items\": {{")?;
    serialize(list_def.t(), &[], writer)?;
    write!(writer, "}}")?;
    Ok(())
}

/// Serialize a slice definition to JSON schema format.
fn serialize_slice<W: Write>(slice_def: facet::SliceDef, writer: &mut W) -> std::io::Result<()> {
    write!(writer, "\"type\": \"array\",")?;
    write!(writer, "\"items\": {{")?;
    serialize(slice_def.t(), &[], writer)?;
    write!(writer, "}}")?;
    Ok(())
}

/// Serialize an array definition to JSON schema format.
fn serialize_array<W: Write>(array_def: facet::ArrayDef, writer: &mut W) -> std::io::Result<()> {
    write!(writer, "\"type\": \"array\",")?;
    write!(writer, "\"minItems\": {},", array_def.n)?;
    write!(writer, "\"maxItems\": {},", array_def.n)?;
    write!(writer, "\"items\": {{")?;
    serialize(array_def.t(), &[], writer)?;
    write!(writer, "}}")?;
    Ok(())
}

/// Serialize an option definition to JSON schema format.
fn serialize_option<W: Write>(
    _option_def: facet::OptionDef,
    writer: &mut W,
) -> std::io::Result<()> {
    write!(writer, "\"type\": \"[]\",")?;
    unimplemented!("serialize_option");
}
