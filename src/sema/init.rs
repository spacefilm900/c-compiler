//! Initializers: shaping brace-enclosed initializer lists onto objects
//! (C11 6.7.9) and folding static initializers to constant stores.

use super::*;

impl Sema<'_> {
    pub(super) fn shape_initializer(
        &mut self,
        ty: &CType,
        init: &Init,
        line: usize,
        column: usize,
    ) -> Result<Shaped, Diagnostic> {
        // A compound literal inside this initializer shapes its own
        // initializer, with its own union member choices.
        let outer = std::mem::take(&mut self.union_choice);
        let outer_overridden = std::mem::take(&mut self.overridden);
        let shaped = self.shape_initializer_inner(ty, init, line, column);
        self.union_choice = outer;
        let overridden = std::mem::replace(&mut self.overridden, outer_overridden);
        let (ty, stores) = shaped?;
        Ok((ty, stores, overridden))
    }

    /// C11 6.7.9p4: every expression in the initializer of an object with
    /// static storage duration is a constant expression, including one whose
    /// store a later designator overrode. Its value is not used.
    pub(super) fn fold_overridden(
        &self,
        name: &str,
        overridden: &[ElemStore],
    ) -> Result<(), Diagnostic> {
        for store in overridden {
            self.fold_one(name, &store.expr, &store.ty)?;
        }
        Ok(())
    }

    pub(super) fn shape_initializer_inner(
        &mut self,
        ty: &CType,
        init: &Init,
        line: usize,
        column: usize,
    ) -> Result<(CType, Vec<ElemStore>), Diagnostic> {
        let mut ty = ty.clone();
        if let CType::Array(elem, None) = &ty {
            let elem = (**elem).clone();
            if let Some((bytes, enc)) = string_payload(init) {
                if string_elem_ok(&elem, enc) {
                    let unit = enc.unit() as u64;
                    ty = CType::array(elem.clone(), Some(bytes.len() as u64 / unit + 1));
                } else if matches!(init, Init::Expr(_)) {
                    let (line, column) = init_pos(init);
                    return self.err(
                        line,
                        column,
                        "a string literal cannot initialize this array",
                    );
                }
                // A brace list around a string, with a non-character element,
                // falls through to element-list length inference.
            }
            if ty.array_len().is_none() {
                let Init::List {
                    items,
                    line,
                    column,
                } = init
                else {
                    let (line, column) = init_pos(init);
                    return self.err(line, column, "array initializer must be a brace list");
                };
                let count = self.infer_array_len(&elem, items, *line, *column)?;
                ty = CType::array(elem, Some(count));
            }
        }
        // Length inference above walked the same items into scratch stores,
        // every element at offset 0. Its member choices and replaced stores
        // belong to that scratch walk, not to this one.
        self.union_choice.clear();
        self.overridden.clear();
        let mut stores = Vec::new();
        self.fill_object(&ty, init, 0, &mut stores, line, column)?;
        if stores.is_empty() && !ty.is_array() {
            return self.err(line, column, "empty initializer list");
        }
        Ok((ty, stores))
    }

    /// Fold an array designator after typing it, so `sizeof` operands are
    /// typed before evaluation (C11 6.7.9p6, 6.5.3.4).
    pub(super) fn designator_index(
        &mut self,
        expr: &Expr,
        line: usize,
        column: usize,
    ) -> Result<u64, Diagnostic> {
        let mut expr = expr.clone();
        self.check_expr(&mut expr)?;
        let value = match eval_const_expr(&expr) {
            Ok(value) if value.ty.is_integer() => value,
            _ => {
                return self.err(line, column, "designator is not an integer constant");
            }
        };
        if value.ty.is_signed() {
            let signed = sign_extend(value.bits, value.ty.width());
            if signed < 0 {
                return self.err(line, column, "array designator is negative");
            }
            match u64::try_from(signed) {
                Ok(bits) => Ok(bits),
                Err(_) => self.err(line, column, "array designator is too large"),
            }
        } else {
            match u64::try_from(value.bits) {
                Ok(bits) => Ok(bits),
                Err(_) => self.err(line, column, "array designator is too large"),
            }
        }
    }

    pub(super) fn infer_array_len(
        &mut self,
        elem: &CType,
        items: &[Init],
        line: usize,
        column: usize,
    ) -> Result<u64, Diagnostic> {
        if items.is_empty() {
            return self.err(line, column, "empty initializer list");
        }
        let mut pos = 0usize;
        let mut index = 0u64;
        let mut count = 0u64;
        let mut scratch = Vec::new();
        while pos < items.len() {
            if let Init::Designated {
                designators,
                line: dline,
                column: dcol,
                ..
            } = &items[pos]
            {
                let Some(Designator::Index { expr, line, column }) = designators.first() else {
                    return self.err(*dline, *dcol, "array designator is required");
                };
                let at = self.designator_index(expr, *line, *column)?;
                if at > 1_000_000 {
                    return self.err(*line, *column, "array designator is too large");
                }
                let Init::Designated { init: first, .. } = &items[pos] else {
                    unreachable!("designated item");
                };
                let elided = designators.len() == 1
                    && !elem.is_scalar()
                    && matches!(first.as_ref(), Init::Expr(_))
                    && !self.opens_subobject(elem, first)
                    && !init_children(elem, false).is_empty();
                if designators.len() > 1 || elided {
                    let rest = designators[1..].to_vec();
                    let dline = *dline;
                    let dcol = *dcol;
                    let first = first.as_ref().clone();
                    let mut scratch = Vec::new();
                    let mut reach = Reach::new(0, self.store_drops);
                    let mut frames = if elided {
                        Vec::new()
                    } else {
                        let (child, frames) = self.seek_designator(elem, 0, &rest, dline, dcol)?;
                        let mut frames = frames;
                        if self.elides_into(&child, &first) {
                            let child_base =
                                frames.last().map(|frame| frame.base).unwrap_or(0) + child.offset;
                            frames.push(InitFrame {
                                children: init_children(&child.ty, false),
                                index: 0,
                                base: child_base,
                                union: union_key(&child.ty),
                            });
                            self.positional_one(&first, &mut frames, &mut scratch, &mut reach)?;
                        } else if !advance_frames(&mut frames) {
                            frames.clear();
                        }
                        frames
                    };
                    if elided {
                        frames.push(InitFrame {
                            children: init_children(elem, false),
                            index: 0,
                            base: 0,
                            union: union_key(elem),
                        });
                        self.positional_one(&first, &mut frames, &mut scratch, &mut reach)?;
                    }
                    pos += 1;
                    while pos < items.len()
                        && !frames.is_empty()
                        && !matches!(items[pos], Init::Designated { .. })
                    {
                        self.positional_one(&items[pos], &mut frames, &mut scratch, &mut reach)?;
                        pos += 1;
                    }
                    count = count.max(at + 1);
                    index = at + 1;
                    continue;
                }
                index = at;
                pos += 1;
            } else {
                let start = pos;
                if self.opens_subobject(elem, &items[pos]) {
                    if !elem.is_scalar() {
                        self.fill_object(elem, &items[pos], 0, &mut scratch, line, column)?;
                    }
                    pos += 1;
                } else {
                    self.fill_unbraced(elem, items, &mut pos, 0, &mut scratch, (line, column))?;
                }
                if pos == start {
                    return self.err(line, column, "empty initializer list");
                }
            }
            index += 1;
            count = count.max(index);
        }
        Ok(count)
    }

    pub(super) fn fill_object(
        &mut self,
        ty: &CType,
        init: &Init,
        base: u64,
        stores: &mut Vec<ElemStore>,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        if let Init::List { items, .. } = init {
            if items
                .iter()
                .any(|item| matches!(item, Init::Designated { .. }))
            {
                return self.fill_mixed(ty, items, base, stores);
            }
        }
        if ty.is_scalar() {
            let expr = self.scalar_expr(init)?;
            stores.push(ElemStore {
                offset: base,
                ty: ty.clone(),
                expr,
                bits: None,
            });
            return Ok(());
        }
        if let Init::Expr(expr) = init {
            if ty.is_record() {
                if string_payload(init).is_some() {
                    return self.err(line, column, "struct initializer must be a brace list");
                }
                stores.push(ElemStore {
                    offset: base,
                    ty: ty.clone(),
                    expr: expr.clone(),
                    bits: None,
                });
                return Ok(());
            }
        }
        if let Some((bytes, enc)) = string_payload(init) {
            let bytes = bytes.to_vec();
            if string_inits(ty, enc) {
                return self.fill_from_string(ty, &bytes, enc, base, stores, init);
            }
            if matches!(init, Init::Expr(_)) {
                let (line, column) = init_pos(init);
                return self.err(
                    line,
                    column,
                    "a string literal cannot initialize this array",
                );
            }
        }
        if ty.is_record() {
            return self.fill_struct(ty, init, base, stores, line, column);
        }
        let CType::Array(elem, Some(len)) = ty else {
            return self.err(line, column, "array initializer must be a brace list");
        };
        let Init::List {
            items,
            line,
            column,
        } = init
        else {
            let (line, column) = init_pos(init);
            return self.err(line, column, "array initializer must be a brace list");
        };
        let elem_size = elem.size().expect("complete array element");
        let mut pos = 0usize;
        let mut index = 0u64;
        while pos < items.len() {
            if index >= *len {
                return self.err(*line, *column, "excess elements in array initializer");
            }
            let off = base + index * elem_size;
            if let Some((bytes, enc)) = string_payload(&items[pos]) {
                let bytes = bytes.to_vec();
                if string_inits(elem, enc) {
                    self.fill_from_string(elem, &bytes, enc, off, stores, &items[pos])?;
                    pos += 1;
                    index += 1;
                    continue;
                }
            }
            if self.opens_subobject(elem, &items[pos]) {
                self.fill_object(elem, &items[pos], off, stores, *line, *column)?;
                pos += 1;
            } else {
                self.fill_unbraced(elem, items, &mut pos, off, stores, (*line, *column))?;
            }
            index += 1;
        }
        Ok(())
    }

    pub(super) fn fill_unbraced(
        &mut self,
        ty: &CType,
        items: &[Init],
        pos: &mut usize,
        base: u64,
        stores: &mut Vec<ElemStore>,
        at: (usize, usize),
    ) -> Result<(), Diagnostic> {
        let (line, column) = at;
        if ty.is_scalar() {
            if *pos >= items.len() {
                return Ok(());
            }
            self.fill_object(ty, &items[*pos], base, stores, line, column)?;
            *pos += 1;
            return Ok(());
        }
        if ty.is_record() {
            let Some(fields) = ty.struct_ref().and_then(StructRef::fields) else {
                return self.err(
                    line,
                    column,
                    format!("variable has incomplete type '{}'", ty.display()),
                );
            };
            // An unbraced union consumes one initializer, the first member.
            // Every member is at offset 0, so walking all of them would store
            // later items on top of the same bytes and steal them from the
            // enclosing initializer.
            let fields = initializable_fields(fields, ty.is_union());
            if let Some(key) = union_key(ty) {
                if *pos < items.len() && !fields.is_empty() {
                    self.select_member(base, key, 0, stores);
                }
            }
            for field in &fields {
                if *pos >= items.len() {
                    break;
                }
                if field.bit_width.is_some() || self.opens_subobject(&field.ty, &items[*pos]) {
                    self.init_field(field, base, &items[*pos], stores)?;
                    *pos += 1;
                } else {
                    let off = base + field.offset;
                    self.fill_unbraced(&field.ty, items, pos, off, stores, (line, column))?;
                }
            }
            return Ok(());
        }
        let CType::Array(elem, Some(len)) = ty else {
            return self.err(line, column, "array initializer must be a brace list");
        };
        let elem_size = elem.size().expect("complete array element");
        for index in 0..*len {
            if *pos >= items.len() {
                break;
            }
            let off = base + index * elem_size;
            if self.opens_subobject(elem, &items[*pos]) {
                self.fill_object(elem, &items[*pos], off, stores, line, column)?;
                *pos += 1;
            } else {
                self.fill_unbraced(elem, items, pos, off, stores, (line, column))?;
            }
        }
        Ok(())
    }

    pub(super) fn fill_struct(
        &mut self,
        ty: &CType,
        init: &Init,
        base: u64,
        stores: &mut Vec<ElemStore>,
        line: usize,
        column: usize,
    ) -> Result<(), Diagnostic> {
        let Some(fields) = ty.struct_ref().and_then(StructRef::fields) else {
            return self.err(
                line,
                column,
                format!("variable has incomplete type '{}'", ty.display()),
            );
        };
        // A brace list for a union initializes the first named member only.
        // Unnamed and zero-width bit-fields are padding, not initializers.
        let fields = initializable_fields(fields, ty.is_union());
        let Init::List {
            items,
            line,
            column,
        } = init
        else {
            let (line, column) = init_pos(init);
            return self.err(line, column, "struct initializer must be a brace list");
        };
        if let Some(key) = union_key(ty) {
            if !items.is_empty() && !fields.is_empty() {
                self.select_member(base, key, 0, stores);
            }
        }
        let mut pos = 0usize;
        for field in &fields {
            if pos >= items.len() {
                break;
            }
            if field.bit_width.is_some() || self.opens_subobject(&field.ty, &items[pos]) {
                self.init_field(field, base, &items[pos], stores)?;
                pos += 1;
            } else {
                let off = base + field.offset;
                self.fill_unbraced(&field.ty, items, &mut pos, off, stores, (*line, *column))?;
            }
        }
        if pos < items.len() {
            let message = if ty.struct_ref().is_some_and(StructRef::has_flexible_member) {
                "initialization of a flexible array member"
            } else {
                "excess elements in struct initializer"
            };
            return self.err(*line, *column, message);
        }
        Ok(())
    }

    /// One initializer element covers this subobject: a brace list, a scalar,
    /// a string for a character array, or an expression of the struct type.
    pub(super) fn opens_subobject(&mut self, ty: &CType, init: &Init) -> bool {
        takes_one_init(ty, init) || self.covers_struct(ty, init)
    }

    /// An expression whose type is this struct initializes the whole field,
    /// instead of the first scalar inside it (`struct Line l = {p, q}`).
    pub(super) fn covers_struct(&mut self, ty: &CType, init: &Init) -> bool {
        if !ty.unqualified().is_record() {
            return false;
        }
        let Init::Expr(expr) = init else {
            return false;
        };
        if string_payload(init).is_some() {
            return false;
        }
        let mut probe = expr.clone();
        if self.check_expr(&mut probe).is_err() {
            return false;
        }
        if self.decay(&mut probe).is_err() {
            return false;
        }
        probe.ty().is_some_and(|src| {
            src.unqualified().is_record() && compatible(&ty.unqualified(), &src.unqualified())
        })
    }

    pub(super) fn scalar_expr(&self, init: &Init) -> Result<Expr, Diagnostic> {
        match init {
            Init::Expr(expr) => Ok(expr.clone()),
            Init::Designated { line, column, .. } => {
                self.err(*line, *column, "designator in a scalar initializer")
            }
            Init::List {
                items,
                line,
                column,
            } => {
                if items.len() != 1 {
                    let message = if items.is_empty() {
                        "empty initializer list"
                    } else {
                        "excess elements in scalar initializer"
                    };
                    return self.err(*line, *column, message);
                }
                match &items[0] {
                    Init::Expr(expr) => Ok(expr.clone()),
                    Init::Designated { line, column, .. } => {
                        self.err(*line, *column, "designator in a scalar initializer")
                    }
                    Init::List { line, column, .. } => {
                        self.err(*line, *column, "excess braces around scalar initializer")
                    }
                }
            }
        }
    }

    pub(super) fn init_field(
        &mut self,
        field: &StructField,
        base: u64,
        init: &Init,
        stores: &mut Vec<ElemStore>,
    ) -> Result<(), Diagnostic> {
        if let Some(width) = field.bit_width.filter(|width| *width > 0) {
            let expr = self.scalar_expr(init)?;
            let declared = field.ty.as_integer().expect("bit-field type is an integer");
            stores.push(ElemStore {
                offset: base + field.offset,
                ty: CType::from_name(bitfield_value_type(declared, width)),
                expr,
                bits: Some(BitStore {
                    abs_bit: (base as u32)
                        .saturating_mul(8)
                        .saturating_add(field.bit_offset),
                    width,
                    container_bits: (field.ty.size().unwrap_or(1) * 8) as u32,
                    signed_field: declared.is_signed(),
                    declared,
                }),
            });
            return Ok(());
        }
        let (line, column) = init_pos(init);
        self.fill_object(&field.ty, init, base + field.offset, stores, line, column)
    }

    /// A brace list that contains at least one designator.
    ///
    /// Designators are relative to this brace's object. After one, the next
    /// positional initializer continues at the following subobject, including
    /// through the elements of an array that a designator stepped into.
    pub(super) fn fill_mixed(
        &mut self,
        ty: &CType,
        items: &[Init],
        base: u64,
        stores: &mut Vec<ElemStore>,
    ) -> Result<(), Diagnostic> {
        let mut frames = vec![InitFrame {
            children: init_children(ty, false),
            index: 0,
            base,
            union: union_key(ty),
        }];
        let mut reach = Reach::new(stores.len(), self.store_drops);
        for item in items {
            if let Init::Designated {
                designators,
                init,
                line,
                column,
            } = item
            {
                let (child, mut frames_at) =
                    self.seek_designator(ty, base, designators, *line, *column)?;
                self.select_frames(&frames_at, stores);
                if self.elides_into(&child, init) {
                    // C11 6.7.9p17, p20: a designated aggregate whose
                    // initializer has no braces starts brace elision at its
                    // first scalar; later positional items continue there.
                    let child_base =
                        frames_at.last().map(|frame| frame.base).unwrap_or(0) + child.offset;
                    frames_at.push(InitFrame {
                        children: init_children(&child.ty, false),
                        index: 0,
                        base: child_base,
                        union: union_key(&child.ty),
                    });
                    self.positional_one(init, &mut frames_at, stores, &mut reach)?;
                    frames = frames_at;
                    continue;
                }
                if !child.ty.is_scalar() && !child.field.as_ref().is_some_and(|f| f.is_bitfield()) {
                    let child_base =
                        frames_at.last().map(|frame| frame.base).unwrap_or(0) + child.offset;
                    self.replace_subobject(&child.ty, child_base, stores, &mut reach);
                }
                self.init_child(&child, &frames_at, init, stores)?;
                frames = frames_at;
                if !advance_frames(&mut frames) {
                    // The designated object was the last subobject. A later
                    // positional initializer is excess; a later designator
                    // starts over from this brace.
                }
                continue;
            }
            self.positional_one(item, &mut frames, stores, &mut reach)?;
        }
        Ok(())
    }

    /// C11 6.7.9p19: an initializer for a whole aggregate subobject (a brace
    /// list, a string literal for a character array, or an expression of
    /// the subobject's struct or union type) overrides every earlier
    /// initializer inside it, and the elements its own list leaves out are
    /// zero, not what an earlier list gave them. So `{ .a = {1, 2}, .a = {3} }`
    /// leaves `a[1]` zero, while `{ .a = {1, 2}, .a[0] = 3 }` keeps it 2.
    /// Earlier stores inside the subobject are dropped; their expressions are
    /// still checked, as for an overridden union member.
    pub(super) fn replace_subobject(
        &mut self,
        ty: &CType,
        base: u64,
        stores: &mut Vec<ElemStore>,
        reach: &mut Reach,
    ) {
        let Some(size) = ty.size().filter(|size| *size > 0) else {
            return;
        };
        if !reach.may_overlap(base, stores, self.store_drops) {
            return;
        }
        let end = base + size;
        let tail = stores.split_off(reach.start);
        let (gone, kept): (Vec<_>, Vec<_>) = tail
            .into_iter()
            .partition(|store| store_within(store, base, end));
        stores.extend(kept);
        if !gone.is_empty() {
            self.overridden.extend(gone);
            self.store_drops += 1;
        }
    }

    /// Whether a designated subobject is an aggregate initialized without
    /// braces, by its first scalar (`{ .point = 1, 2 }`). A brace list, a
    /// string for a character array, or an expression of the member's own
    /// struct or union type initializes the whole subobject instead.
    pub(super) fn elides_into(&mut self, child: &InitChild, init: &Init) -> bool {
        if child.ty.is_scalar() || child.field.as_ref().is_some_and(|f| f.is_bitfield()) {
            return false;
        }
        if !matches!(init, Init::Expr(_)) || self.opens_subobject(&child.ty, init) {
            return false;
        }
        !init_children(&child.ty, false).is_empty()
    }

    pub(super) fn seek_designator(
        &mut self,
        ty: &CType,
        base: u64,
        designators: &[Designator],
        line: usize,
        column: usize,
    ) -> Result<(InitChild, Vec<InitFrame>), Diagnostic> {
        let mut current = ty.clone();
        let mut current_base = base;
        let mut frames = Vec::new();
        if designators.is_empty() {
            return self.err(line, column, "designator does not match this type");
        }
        for (nth, designator) in designators.iter().enumerate() {
            if let Designator::Field { name, line, column } = designator {
                if current.unqualified().is_array() {
                    return self.err(*line, *column, "field designator on an array");
                }
                let Some(path) = field_designator_path(&current, name) else {
                    if flexible_member_named(&current, name) {
                        return self.err(
                            *line,
                            *column,
                            "initialization of a flexible array member",
                        );
                    }
                    return self.err(*line, *column, format!("no member named '{name}'"));
                };
                let mut chosen = None;
                for (step, index) in path.iter().enumerate() {
                    let children = init_children(&current, true);
                    if children.is_empty() || *index >= children.len() {
                        return self.err(*line, *column, "designator does not match this type");
                    }
                    let child = children[*index].clone();
                    frames.push(InitFrame {
                        children,
                        index: *index,
                        base: current_base,
                        union: union_key(&current),
                    });
                    if step + 1 == path.len() {
                        chosen = Some(child);
                        break;
                    }
                    current_base += child.offset;
                    current = child.ty.clone();
                }
                let child = chosen.expect("designator path");
                if nth + 1 == designators.len() {
                    return Ok((child, frames));
                }
                current_base += child.offset;
                current = child.ty;
                continue;
            }
            let children = init_children(&current, true);
            if children.is_empty() {
                return self.err(line, column, "designator does not match this type");
            }
            let index = match designator {
                Designator::Field { .. } => unreachable!("field designators are handled above"),
                Designator::Index { expr, line, column } => {
                    if !current.unqualified().is_array() {
                        return self.err(*line, *column, "array designator on a non-array");
                    }
                    let index = self.designator_index(expr, *line, *column)?;
                    if index as usize >= children.len() {
                        return self.err(*line, *column, "array designator is out of range");
                    }
                    index as usize
                }
            };
            let child = children[index].clone();
            frames.push(InitFrame {
                children,
                index,
                base: current_base,
                union: union_key(&current),
            });
            if nth + 1 == designators.len() {
                return Ok((child, frames));
            }
            current_base += child.offset;
            current = child.ty;
        }
        self.err(line, column, "designator does not match this type")
    }

    pub(super) fn init_child(
        &mut self,
        child: &InitChild,
        frames: &[InitFrame],
        init: &Init,
        stores: &mut Vec<ElemStore>,
    ) -> Result<(), Diagnostic> {
        let base = frames.last().map(|frame| frame.base).unwrap_or(0);
        if let Some(field) = &child.field {
            self.init_field(field, base, init, stores)
        } else {
            let (line, column) = init_pos(init);
            self.fill_object(&child.ty, init, base + child.offset, stores, line, column)
        }
    }

    pub(super) fn positional_one(
        &mut self,
        item: &Init,
        frames: &mut Vec<InitFrame>,
        stores: &mut Vec<ElemStore>,
        reach: &mut Reach,
    ) -> Result<(), Diagnostic> {
        loop {
            let Some(frame) = frames.last() else {
                let (line, column) = init_pos(item);
                return self.err(line, column, "excess elements in initializer");
            };
            if frame.index >= frame.children.len() {
                let (line, column) = init_pos(item);
                return self.err(line, column, "excess elements in initializer");
            }
            let child = frame.children[frame.index].clone();
            let child_base = frame.base + child.offset;
            if let Some(key) = frame.union {
                let (union_base, member) = (frame.base, frame.index);
                self.select_member(union_base, key, member, stores);
            }
            let scalar =
                child.ty.is_scalar() || child.field.as_ref().is_some_and(|f| f.is_bitfield());
            if scalar || self.opens_subobject(&child.ty, item) {
                if !scalar {
                    self.replace_subobject(&child.ty, child_base, stores, reach);
                }
                if let Some(field) = &child.field {
                    if field.is_bitfield() || scalar {
                        self.init_field(field, frame.base, item, stores)?;
                    } else {
                        let (line, column) = init_pos(item);
                        self.fill_object(&child.ty, item, child_base, stores, line, column)?;
                    }
                } else {
                    let (line, column) = init_pos(item);
                    self.fill_object(&child.ty, item, child_base, stores, line, column)?;
                }
                if !advance_frames(frames) {
                    frames.clear();
                }
                return Ok(());
            }
            let nested = init_children(&child.ty, false);
            if nested.is_empty() {
                let (line, column) = init_pos(item);
                return self.err(line, column, "excess elements in initializer");
            }
            frames.push(InitFrame {
                children: nested,
                index: 0,
                base: child_base,
                union: union_key(&child.ty),
            });
        }
    }

    /// C11 6.7.9p14. The terminating NUL is stored when the array is larger
    /// than the literal or has unknown size (already made known). An array
    /// whose length equals the literal, not counting that NUL, is initialized
    /// without it. A longer literal is a diagnostic.
    pub(super) fn fill_from_string(
        &self,
        ty: &CType,
        bytes: &[u8],
        enc: StrEnc,
        base: u64,
        stores: &mut Vec<ElemStore>,
        init: &Init,
    ) -> Result<(), Diagnostic> {
        let (line, column) = init_pos(init);
        let Some(len) = ty.array_len() else {
            return self.err(line, column, "array size missing");
        };
        let unit = enc.unit();
        let units: Vec<u64> = bytes
            .chunks(unit)
            .map(|chunk| {
                let mut acc = 0u64;
                for (index, byte) in chunk.iter().enumerate() {
                    acc |= u64::from(*byte) << (8 * index);
                }
                acc
            })
            .collect();
        if units.len() as u64 > len {
            return self.err(line, column, "initializer string is too long for the array");
        }
        let elem = ty.array_elem().expect("character array").clone();
        let elem_size = elem.size().expect("character element");
        let mut data = units;
        if (data.len() as u64) < len {
            data.push(0);
        }
        for (index, value) in data.iter().enumerate() {
            if index as u64 >= len {
                break;
            }
            stores.push(ElemStore {
                offset: base + index as u64 * elem_size,
                ty: elem.clone(),
                expr: Expr::int(*value, TypeName::Int, line, column),
                bits: None,
            });
        }
        Ok(())
    }

    /// Record that the initializer for the union at byte `base` names
    /// `member`. C11 6.7.9p17 and p19: when a designator names a different
    /// member from the one initialized so far, that member is the one the
    /// union holds, and its subobjects not named later are initialized as
    /// for static storage (zero). Stores already made inside the union
    /// belonged to the earlier member, so they are dropped, together with
    /// the member choices of unions nested inside it.
    pub(super) fn select_member(
        &mut self,
        base: u64,
        key: UnionKey,
        member: usize,
        stores: &mut Vec<ElemStore>,
    ) {
        let (id, size) = key;
        let end = base + size;
        let earlier = self
            .union_choice
            .iter()
            .position(|(at, (other, _), _)| *at == base && *other == id);
        let Some(pos) = earlier else {
            self.union_choice.push((base, key, member));
            return;
        };
        if self.union_choice[pos].2 == member {
            return;
        }
        // The earlier member's stores are not part of the object, but their
        // expressions are still checked (see `Shaped`).
        let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(stores)
            .into_iter()
            .partition(|store| store_within(store, base, end));
        *stores = kept;
        if !gone.is_empty() {
            self.store_drops += 1;
        }
        self.overridden.extend(gone);
        self.union_choice[pos].2 = member;
        // A nested union is first chosen after the union around it, so the
        // choices after this one that lie inside it are its descendants. An
        // enclosing union of the same offset and size comes before it and
        // keeps its choice.
        let mut index = 0;
        self.union_choice.retain(|(at, (_, len), _)| {
            let keep = index <= pos || !(*at >= base && *at + *len <= end);
            index += 1;
            keep
        });
    }

    /// [`Self::select_member`] for every union a designator or positional
    /// walk passes through, outermost first.
    pub(super) fn select_frames(&mut self, frames: &[InitFrame], stores: &mut Vec<ElemStore>) {
        for frame in frames {
            if let Some(key) = frame.union {
                self.select_member(frame.base, key, frame.index, stores);
            }
        }
    }

    pub(super) fn check_stores(&mut self, stores: &mut [ElemStore]) -> Result<(), Diagnostic> {
        for store in stores {
            self.check_expr(&mut store.expr)?;
            self.decay(&mut store.expr)?;
            self.can_assign(&store.ty.clone(), &store.expr)?;
        }
        Ok(())
    }

    pub(super) fn fold_stores(
        &self,
        name: &str,
        ty: &CType,
        stores: &[ElemStore],
    ) -> Result<ConstInit, Diagnostic> {
        if ty.is_union() || ty.record_has_bitfield() {
            return self.fold_bit_object(name, ty, stores);
        }
        // A plain member of a nested bit-field record has no bit store. The
        // record is still a byte image, so that member has to be written into
        // it. A bit store is also enough on its own: a union's range walk
        // sees only the first member, and a later bit-field member would
        // otherwise be missed.
        let mut nested = Vec::new();
        bit_record_ranges(ty, 0, &mut nested);
        if stores.iter().any(|store| store.bits.is_some()) || !nested.is_empty() {
            return self.fold_nested_bits(name, ty, stores);
        }
        let mut unions = UnionImages::new(ty);
        let mut folded = Vec::new();
        for store in stores {
            let value = self.fold_one(name, &store.expr, &store.ty)?;
            if !unions.absorb(store, &value)? {
                folded.push((store.offset, value));
            }
        }
        unions.finish(name, self, &mut folded)?;
        let value = assemble(ty, &folded, 0);
        if !union_bytes_ok(ty, &value) {
            return self.err(
                stores.first().map(|store| store.expr.line()).unwrap_or(1),
                stores.first().map(|store| store.expr.column()).unwrap_or(1),
                format!("initializer of '{name}' uses an address in a union"),
            );
        }
        Ok(value)
    }

    pub(super) fn fold_bit_object(
        &self,
        name: &str,
        ty: &CType,
        stores: &[ElemStore],
    ) -> Result<ConstInit, Diagnostic> {
        let size = ty.size().unwrap_or(0) as usize;
        let mut image = ByteImage::zeroed(size);
        for store in stores {
            if let Some(bits) = &store.bits {
                let fold_ty = if bits.declared == TypeName::Bool {
                    CType::from_name(TypeName::Bool)
                } else {
                    store.ty.clone()
                };
                let value = self.fold_one(name, &store.expr, &fold_ty)?;
                let ConstInit::Int(raw) = value else {
                    return self.err(
                        store.expr.line(),
                        store.expr.column(),
                        format!("initializer of '{name}' is not a constant expression"),
                    );
                };
                image.pack_bits(bits.abs_bit, bits.width, raw);
            } else {
                let value = self.fold_one(name, &store.expr, &store.ty)?;
                if !image.write(&store.ty, &value, store.offset as usize) {
                    let kind = if ty.is_union() {
                        "union"
                    } else {
                        "bit-field struct"
                    };
                    return self.err(
                        store.expr.line(),
                        store.expr.column(),
                        format!("initializer of '{name}' uses an address in a {kind}"),
                    );
                }
            }
        }
        Ok(image.into_const())
    }

    /// A file-scope initializer whose top type is an ordinary struct or array,
    /// with a bit-field struct nested inside it. The bit-field subobjects
    /// become raw bytes. A sibling field that is an address stays a relocation.
    pub(super) fn fold_nested_bits(
        &self,
        name: &str,
        ty: &CType,
        stores: &[ElemStore],
    ) -> Result<ConstInit, Diagnostic> {
        let size = ty.size().unwrap_or(0) as usize;
        let mut image = ByteImage::zeroed(size);
        let mut ranges = Vec::new();
        bit_record_ranges(ty, 0, &mut ranges);
        let mut unions = UnionImages::new(ty);
        let mut folded = Vec::new();
        for store in stores {
            if let Some(bits) = &store.bits {
                let fold_ty = if bits.declared == TypeName::Bool {
                    CType::from_name(TypeName::Bool)
                } else {
                    store.ty.clone()
                };
                let value = self.fold_one(name, &store.expr, &fold_ty)?;
                let ConstInit::Int(raw) = value else {
                    return self.err(
                        store.expr.line(),
                        store.expr.column(),
                        format!("initializer of '{name}' is not a constant expression"),
                    );
                };
                image.pack_bits(bits.abs_bit, bits.width, raw);
            } else {
                let value = self.fold_one(name, &store.expr, &store.ty)?;
                let inside = ranges
                    .iter()
                    .any(|(start, end)| store.offset >= *start && store.offset < *end);
                if inside {
                    // The nested record is a byte image. A plain member,
                    // including an address, is written into that image.
                    if !image.write(&store.ty, &value, store.offset as usize) {
                        return self.err(
                            store.expr.line(),
                            store.expr.column(),
                            format!(
                                "initializer of '{name}' uses an address in a bit-field struct"
                            ),
                        );
                    }
                } else if !unions.absorb(store, &value)? {
                    folded.push((store.offset, value));
                }
            }
        }
        unions.finish(name, self, &mut folded)?;
        if !splice_bit_blobs(ty, 0, &image, &mut folded) {
            return self.err(
                stores.first().map(|store| store.expr.line()).unwrap_or(1),
                stores.first().map(|store| store.expr.column()).unwrap_or(1),
                format!("initializer of '{name}' uses an address in a bit-field struct"),
            );
        }
        let value = assemble(ty, &folded, 0);
        if !union_bytes_ok(ty, &value) {
            return self.err(
                stores.first().map(|store| store.expr.line()).unwrap_or(1),
                stores.first().map(|store| store.expr.column()).unwrap_or(1),
                format!("initializer of '{name}' uses an address in a union"),
            );
        }
        Ok(value)
    }

    pub(super) fn fold_one(
        &self,
        name: &str,
        expr: &Expr,
        dest: &CType,
    ) -> Result<ConstInit, Diagnostic> {
        if let Some(dest_int) = dest.as_integer() {
            match eval_num(expr) {
                Ok(NumConst::Int(value)) => {
                    return Ok(ConstInit::Int(convert_bits(value.bits, value.ty, dest_int)));
                }
                Ok(NumConst::Real { bits, kind }) => {
                    return match crate::float::real_to_int(bits, kind, dest_int) {
                        Some(converted) => Ok(ConstInit::Int(converted)),
                        None => self.err(
                            expr.line(),
                            expr.column(),
                            format!("initializer of '{name}' overflows '{}'", dest_int.as_str()),
                        ),
                    };
                }
                Err(_) => {}
            }
            if let Expr::Cast {
                ty_name,
                expr: inner,
                ..
            } = expr
            {
                if let Some(cast_int) = ty_name.as_integer() {
                    if let Ok(ptr) = eval_ptr(inner) {
                        // A file-scope address is not a number until link time.
                        // C11 6.6p7 has no integer form of an address constant;
                        // 6.6p10 lets an implementation accept one, and only
                        // the pointer-sized form is accepted here: a 64-bit
                        // cast into a 64-bit object keeps every bit
                        // (`ptrtoint` to i64). Truncating the address, or
                        // widening it to `__int128`, cannot be folded.
                        if matches!(ptr, PtrConst::Addr { .. } | PtrConst::Label { .. }) {
                            let narrowest = cast_int.width().min(dest_int.width());
                            let widest = cast_int.width().max(dest_int.width());
                            if narrowest < 64 {
                                return self.err(
                                    expr.line(),
                                    expr.column(),
                                    format!("initializer of '{name}' truncates an address"),
                                );
                            }
                            if widest > 64 {
                                return self.err(
                                    expr.line(),
                                    expr.column(),
                                    format!(
                                        "initializer of '{name}' converts an address to an \
                                         integer wider than a pointer, which is not a \
                                         constant expression"
                                    ),
                                );
                            }
                        }
                        return Ok(match ptr_as_int(ptr, cast_int) {
                            ConstInit::Int(bits) => {
                                ConstInit::Int(convert_bits(bits, cast_int, dest_int))
                            }
                            other => other,
                        });
                    }
                }
            }
            return self.const_diag(name, expr);
        }
        if let Some(kind) = dest.float_kind() {
            return match eval_num(expr) {
                Ok(value) => match value.to_real(kind) {
                    Some(bits) => Ok(ConstInit::Float(bits)),
                    None => self.err(
                        expr.line(),
                        expr.column(),
                        format!("initializer of '{name}' is not a constant expression"),
                    ),
                },
                Err(_) => self.const_diag(name, expr),
            };
        }
        if dest.is_pointer() {
            if is_null_pointer_constant(expr) {
                return Ok(ConstInit::Ptr(PtrConst::Null));
            }
            return match eval_ptr(expr) {
                Ok(ptr) => Ok(ConstInit::Ptr(ptr)),
                Err(_) => self.const_diag(name, expr),
            };
        }
        if dest.is_record() {
            return self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' is not a constant expression"),
            );
        }
        self.err(
            expr.line(),
            expr.column(),
            format!("initializer of '{name}' is not a constant expression"),
        )
    }

    pub(super) fn const_diag(&self, name: &str, expr: &Expr) -> Result<ConstInit, Diagnostic> {
        match eval_const_expr(expr) {
            Err(ConstError::DivZero) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' divides by zero"),
            ),
            Err(ConstError::Overflow(ty)) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' overflows '{}'", ty.as_str()),
            ),
            Err(ConstError::Shift) => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' uses an undefined shift"),
            ),
            _ => self.err(
                expr.line(),
                expr.column(),
                format!("initializer of '{name}' is not a constant expression"),
            ),
        }
    }
}

pub(super) struct InitChild {
    pub(super) ty: CType,
    pub(super) offset: u64,
    pub(super) field: Option<StructField>,
}

impl Clone for InitChild {
    fn clone(&self) -> Self {
        Self {
            ty: self.ty.clone(),
            offset: self.offset,
            field: self.field.clone(),
        }
    }
}

/// How far the stores of one brace list reach, so that a whole-subobject
/// initializer searches for earlier stores to replace only when one can lie
/// inside it ([`Sema::replace_subobject`]). Stores are mostly made in
/// increasing offset order, so this keeps a long designated list linear.
pub(super) struct Reach {
    /// The first store that belongs to this brace list.
    start: usize,
    /// Stores before this index are counted in `end`.
    seen: usize,
    /// One past the furthest byte a counted store writes.
    end: u64,
    /// [`Sema::store_drops`] when `seen` was last valid. Dropping stores
    /// moves later ones down, so a change means counting again.
    drops: usize,
}

impl Reach {
    pub(super) fn new(start: usize, drops: usize) -> Self {
        Reach {
            start,
            seen: start,
            end: 0,
            drops,
        }
    }

    /// Whether a store of this list may start before `base`'s subobject
    /// ends, that is, whether any store reaches past `base`.
    fn may_overlap(&mut self, base: u64, stores: &[ElemStore], drops: usize) -> bool {
        if drops != self.drops || stores.len() < self.seen {
            self.seen = self.start.min(stores.len());
            self.end = 0;
            self.drops = drops;
        }
        for store in &stores[self.seen..] {
            self.end = self.end.max(store_reach(store));
        }
        self.seen = stores.len();
        self.end > base
    }
}

/// One past the last byte a store writes. A bit-field store counts from the
/// byte of its first bit, as [`store_within`] does.
fn store_reach(store: &ElemStore) -> u64 {
    if let Some(bits) = &store.bits {
        let first = u64::from(bits.abs_bit / 8);
        let bytes = u64::from(bits.width.div_ceil(8)) + 1;
        return first.saturating_add(bytes);
    }
    store
        .offset
        .saturating_add(store.ty.size().unwrap_or(0).max(1))
}

pub(super) struct InitFrame {
    pub(super) children: Vec<InitChild>,
    pub(super) index: usize,
    pub(super) base: u64,
    /// A union initializes one member. The next positional item continues
    /// after the union, not at another member of it. The key identifies the
    /// union type and gives its size, for [`Sema::select_member`].
    pub(super) union: Option<UnionKey>,
}

/// Whether a store lies inside the bytes `base..end`.
pub(super) fn store_within(store: &ElemStore, base: u64, end: u64) -> bool {
    if let Some(bits) = &store.bits {
        let first = u64::from(bits.abs_bit / 8);
        return first >= base && first < end;
    }
    let size = store.ty.size().unwrap_or(0);
    store.offset >= base && store.offset < end && store.offset + size <= end
}

/// A shaped initializer: the object's type (an array's length completed),
/// the stores that make up its value, and the stores of union members that
/// a later designator replaced. Those are not part of the value, but their
/// expressions are still checked like the others (C11 6.7.9p2, p4).
pub(super) type Shaped = (CType, Vec<ElemStore>, Vec<ElemStore>);

/// A union type's identity and size in bytes.
pub(super) type UnionKey = (u32, u64);

pub(super) fn union_key(ty: &CType) -> Option<UnionKey> {
    if !ty.is_union() {
        return None;
    }
    let handle = ty.struct_ref()?;
    Some((handle.id(), ty.size().unwrap_or(0)))
}

/// Indexes through `init_children(..., true)` that reach `name`.
///
/// Anonymous struct and union members are not names, so a designator steps
/// through them to the member they promote.
pub(super) fn field_designator_path(ty: &CType, name: &str) -> Option<Vec<usize>> {
    let children = init_children(ty, true);
    for (index, child) in children.iter().enumerate() {
        if child.field.as_ref().is_some_and(|field| field.name == name) {
            return Some(vec![index]);
        }
        if child
            .field
            .as_ref()
            .is_some_and(StructField::is_anonymous_record)
        {
            if let Some(mut rest) = field_designator_path(&child.ty, name) {
                rest.insert(0, index);
                return Some(rest);
            }
        }
    }
    None
}

pub(super) fn init_children(ty: &CType, union_all: bool) -> Vec<InitChild> {
    if let Some(handle) = ty.struct_ref() {
        let Some(fields) = handle.fields() else {
            return Vec::new();
        };
        let fields = if ty.is_union() && !union_all {
            initializable_fields(fields, true)
        } else {
            initializable_fields(fields, false)
        };
        return fields
            .into_iter()
            .map(|field| InitChild {
                offset: field.offset,
                ty: field.ty.clone(),
                field: Some(field),
            })
            .collect();
    }
    if let CType::Array(elem, Some(len)) = ty.unqualified() {
        let step = elem.size().unwrap_or(0);
        return (0..len)
            .map(|index| InitChild {
                ty: (*elem).clone(),
                offset: index * step,
                field: None,
            })
            .collect();
    }
    Vec::new()
}

pub(super) fn advance_frames(frames: &mut Vec<InitFrame>) -> bool {
    loop {
        let Some(frame) = frames.last_mut() else {
            return false;
        };
        if frame.union.is_some() {
            frames.pop();
            continue;
        }
        frame.index += 1;
        if frame.index < frame.children.len() {
            return true;
        }
        frames.pop();
    }
}

pub(super) fn initializable_fields(fields: Vec<StructField>, union_ty: bool) -> Vec<StructField> {
    let fields: Vec<_> = fields
        .into_iter()
        .filter(|field| {
            field.bit_width != Some(0)
                && !field.is_flexible()
                && (!field.name.is_empty() || field.is_anonymous_record())
        })
        .collect();
    if union_ty {
        fields.into_iter().take(1).collect()
    } else {
        fields
    }
}

pub(super) fn init_pos(init: &Init) -> (usize, usize) {
    match init {
        Init::Expr(expr) => (expr.line(), expr.column()),
        Init::List { line, column, .. } | Init::Designated { line, column, .. } => (*line, *column),
    }
}

pub(super) fn zero_const(ty: &CType) -> ConstInit {
    if ty.is_integer() {
        ConstInit::Int(0)
    } else if ty.is_pointer() {
        ConstInit::Ptr(PtrConst::Null)
    } else {
        ConstInit::Zero
    }
}

/// Bytes of a string literal used as an initializer, including a single pair
/// of braces around that literal (`char s[4] = {"hi"}`).
pub(super) fn string_payload(init: &Init) -> Option<(&[u8], StrEnc)> {
    match init {
        Init::Expr(Expr::Str { bytes, enc, .. }) => Some((bytes, *enc)),
        Init::Designated { .. } => None,
        Init::List { items, .. } => match items.as_slice() {
            [Init::Expr(Expr::Str { bytes, enc, .. })] => Some((bytes, *enc)),
            _ => None,
        },
        Init::Expr(_) => None,
    }
}

/// The element type is the one C11 6.7.9 allows for this encoding.
pub(super) fn string_elem_ok(elem: &CType, enc: StrEnc) -> bool {
    match enc {
        StrEnc::Byte => elem.is_character(),
        StrEnc::Utf16 => elem.as_integer() == Some(TypeName::UShort),
        StrEnc::Wchar => elem.as_integer() == Some(StrEnc::Wchar.elem()),
        StrEnc::Utf32 => elem.as_integer() == Some(TypeName::UInt),
    }
}

pub(super) fn string_inits(ty: &CType, enc: StrEnc) -> bool {
    ty.array_elem()
        .is_some_and(|elem| string_elem_ok(elem, enc))
}

pub(super) fn flexible_member_named(ty: &CType, name: &str) -> bool {
    ty.struct_ref()
        .and_then(StructRef::fields)
        .is_some_and(|fields| {
            fields
                .iter()
                .any(|field| field.name == name && field.is_flexible())
        })
}

pub(super) fn takes_one_init(ty: &CType, init: &Init) -> bool {
    if matches!(init, Init::List { .. }) || ty.is_scalar() {
        return true;
    }
    string_payload(init).is_some_and(|(_, enc)| string_inits(ty, enc))
}

pub(super) fn classify_init(ty: &CType, mut stores: Vec<ElemStore>) -> CheckedInit {
    // A single store of the whole object is a scalar or a struct copy.
    // One store of a field, including the only field, stays an aggregate
    // store at that field's offset.
    let whole = stores.len() == 1
        && (ty.is_scalar()
            || (ty.is_record() && compatible(&stores[0].ty.unqualified(), &ty.unqualified())));
    if whole {
        return CheckedInit::Scalar(stores.remove(0).expr);
    }
    CheckedInit::Aggregate(stores)
}
