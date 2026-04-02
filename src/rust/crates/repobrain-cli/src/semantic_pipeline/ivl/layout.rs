use super::{IVL_POINTER_WIDTH_BITS, LayoutType};

pub(super) fn compute_gep_offset(layout: &LayoutType, indices: &[i64]) -> Result<i64, String> {
    let mut current = layout.clone();
    let mut offset = 0_i64;
    for (position, index) in indices.iter().copied().enumerate() {
        if position == 0 {
            let size = i64::try_from(layout_byte_size(&current)?)
                .map_err(|_| "layout_size_overflow".to_string())?;
            offset = offset
                .checked_add(size.saturating_mul(index))
                .ok_or_else(|| "gep_offset_overflow".to_string())?;
            continue;
        }
        match current {
            LayoutType::Array(_, element) => {
                let element_size = i64::try_from(layout_byte_size(&element)?)
                    .map_err(|_| "layout_size_overflow".to_string())?;
                offset = offset
                    .checked_add(element_size.saturating_mul(index))
                    .ok_or_else(|| "gep_offset_overflow".to_string())?;
                current = *element;
            }
            LayoutType::Struct(fields) => {
                let field_index =
                    usize::try_from(index).map_err(|_| "negative_struct_gep_index".to_string())?;
                offset = offset
                    .checked_add(struct_field_offset(&fields, field_index)?)
                    .ok_or_else(|| "gep_offset_overflow".to_string())?;
                current = fields
                    .get(field_index)
                    .cloned()
                    .ok_or_else(|| "struct_gep_field_out_of_bounds".to_string())?;
            }
            LayoutType::Int(_) | LayoutType::Ptr => {
                let size = i64::try_from(layout_byte_size(&current)?)
                    .map_err(|_| "layout_size_overflow".to_string())?;
                offset = offset
                    .checked_add(size.saturating_mul(index))
                    .ok_or_else(|| "gep_offset_overflow".to_string())?;
            }
            LayoutType::Void => return Err("void_gep_layout".to_string()),
        }
    }
    Ok(offset)
}

pub(super) fn layout_byte_size(layout: &LayoutType) -> Result<u64, String> {
    match layout {
        LayoutType::Int(width) => Ok(u64::from((*width).max(8).div_ceil(8))),
        LayoutType::Ptr => Ok(u64::from(IVL_POINTER_WIDTH_BITS / 8)),
        LayoutType::Array(count, element) => layout_byte_size(element)?
            .checked_mul(*count)
            .ok_or_else(|| "array_layout_size_overflow".to_string()),
        LayoutType::Struct(fields) => struct_layout_byte_size(fields),
        LayoutType::Void => Err("void_layout_size".to_string()),
    }
}

fn struct_field_offset(fields: &[LayoutType], field_index: usize) -> Result<i64, String> {
    let Some(_) = fields.get(field_index) else {
        return Err("struct_gep_field_out_of_bounds".to_string());
    };
    let mut offset = 0_u64;
    for field in fields.iter().take(field_index) {
        let align = layout_align_bytes(field)?;
        offset = round_up_to_alignment(offset, align)?;
        offset = offset
            .checked_add(layout_byte_size(field)?)
            .ok_or_else(|| "struct_layout_size_overflow".to_string())?;
    }
    let align = layout_align_bytes(&fields[field_index])?;
    let aligned = round_up_to_alignment(offset, align)?;
    i64::try_from(aligned).map_err(|_| "layout_size_overflow".to_string())
}

fn struct_layout_byte_size(fields: &[LayoutType]) -> Result<u64, String> {
    let mut offset = 0_u64;
    let mut max_align = 1_u64;
    for field in fields {
        let align = layout_align_bytes(field)?;
        max_align = max_align.max(align);
        offset = round_up_to_alignment(offset, align)?;
        offset = offset
            .checked_add(layout_byte_size(field)?)
            .ok_or_else(|| "struct_layout_size_overflow".to_string())?;
    }
    round_up_to_alignment(offset, max_align)
}

fn layout_align_bytes(layout: &LayoutType) -> Result<u64, String> {
    match layout {
        LayoutType::Int(width) => Ok(u64::from((*width).max(8).div_ceil(8))),
        LayoutType::Ptr => Ok(u64::from(IVL_POINTER_WIDTH_BITS / 8)),
        LayoutType::Array(_, element) => layout_align_bytes(element),
        LayoutType::Struct(fields) => fields.iter().try_fold(1_u64, |max_align, field| {
            Ok(max_align.max(layout_align_bytes(field)?))
        }),
        LayoutType::Void => Err("void_layout_align".to_string()),
    }
}

fn round_up_to_alignment(value: u64, align: u64) -> Result<u64, String> {
    if align == 0 {
        return Err("zero_alignment".to_string());
    }
    let remainder = value % align;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(align - remainder)
        .ok_or_else(|| "alignment_overflow".to_string())
}
