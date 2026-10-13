        // Validate each optional mask before allocating the dense score matrix.
        let query_mask = q_mask
            .map(|mask| expand_full_mask(mask, queries, true))
            .transpose()?;
        let key_mask = kv_mask
            .map(|mask| expand_full_mask(mask, keys, false))
            .transpose()?;
        let available_keys = key_mask.as_ref().map(|mask| mask.any_dim(2, true));
        let supported_rows = match (&query_mask, &available_keys) {
            (Some(query), Some(available)) => Some(query.logical_and(available)),
            (Some(query), None) => Some(query.shallow_clone()),
            (None, Some(available)) => Some(available.shallow_clone()),
            (None, None) => None,
        };

        let mut qk = Tensor::einsum("nlhd,nshd->nlsh", &[queries, keys], None::<&[i64]>);
        if let (Some(key_mask), Some(available)) = (&key_mask, &available_keys) {
            qk = qk.f_masked_fill(&key_mask.logical_not(), f64::NEG_INFINITY)?;
            // Softmax(-inf, ..., -inf) is NaN. Unsupported batches use harmless
            // finite logits, then receive exactly zero attention below.
            qk = qk.f_masked_fill(&available.logical_not(), 0.0)?;
        }

        let softmax_temp = 1.0 / i64_to_f64(queries.size()[3], "attention head dim")?.sqrt();
        let mut attention = (qk * softmax_temp).softmax(2, Kind::Float);
        if let Some(supported) = supported_rows {
            attention = attention.f_masked_fill(&supported.logical_not(), 0.0)?;
        }
