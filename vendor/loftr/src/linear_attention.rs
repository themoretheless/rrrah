// Modified by rrrah (2026-10-12): independent full-attention masks and finite empty support.
// Modified by rrrah (2026-10-12): preserve borrowed values during masking.
use tch::{Kind, Tensor};

use crate::{error::LoftrError, numeric::i64_to_f64};

#[derive(Debug, Clone, Copy)]
pub struct LinearAttention {
    eps: f64,
}

impl Default for LinearAttention {
    fn default() -> Self {
        Self { eps: 1e-6 }
    }
}

impl LinearAttention {
    pub fn forward(
        self,
        queries: &Tensor,
        keys: &Tensor,
        values: &Tensor,
        q_mask: Option<&Tensor>,
        kv_mask: Option<&Tensor>,
    ) -> Result<Tensor, LoftrError> {
        validate_attention_tensors(queries, keys, values)?;

        let mut q = queries.elu() + 1.0;
        let mut k = keys.elu() + 1.0;
        let mut v = values.shallow_clone();

        if let Some(mask) = q_mask {
            let mask = expand_attention_mask(mask, queries)?;
            q *= mask;
        }
        if let Some(mask) = kv_mask {
            let mask = expand_attention_mask(mask, keys)?;
            k *= &mask;
            // A shallow clone still aliases the borrowed values (and caller aliases).
            // Allocate the masked result so read-only inputs and leaf gradients survive.
            v = &v * mask;
        }

        let value_length = i64_to_f64(values.size()[1], "attention value length")?;
        let v = &v / value_length;
        let kv = Tensor::einsum("nshd,nshv->nhdv", &[&k, &v], None::<&[i64]>);
        let z = (Tensor::einsum(
            "nlhd,nhd->nlh",
            &[&q, &k.sum_dim_intlist([1].as_slice(), false, Kind::Float)],
            None::<&[i64]>,
        ) + self.eps)
            .reciprocal();
        Ok(
            (Tensor::einsum("nlhd,nhdv,nlh->nlhv", &[&q, &kv, &z], None::<&[i64]>) * value_length)
                .contiguous(),
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FullAttention {
    use_dropout: bool,
    attention_dropout: f64,
}

impl Default for FullAttention {
    fn default() -> Self {
        Self {
            use_dropout: false,
            attention_dropout: 0.1,
        }
    }
}

impl FullAttention {
    pub fn forward(
        self,
        queries: &Tensor,
        keys: &Tensor,
        values: &Tensor,
        q_mask: Option<&Tensor>,
        kv_mask: Option<&Tensor>,
    ) -> Result<Tensor, LoftrError> {
        validate_attention_tensors(queries, keys, values)?;

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
        if self.use_dropout && self.attention_dropout > 0.0 {
            attention = attention.dropout(self.attention_dropout, false);
        }
        Ok(Tensor::einsum("nlsh,nshd->nlhd", &[&attention, values], None::<&[i64]>).contiguous())
    }
}

fn validate_attention_tensors(
    queries: &Tensor,
    keys: &Tensor,
    values: &Tensor,
) -> Result<(), LoftrError> {
    let q_dims = queries.size();
    let k_dims = keys.size();
    let v_dims = values.size();
    if q_dims.len() != 4 || k_dims.len() != 4 || v_dims.len() != 4 {
        return Err(LoftrError::InvalidConfig(format!(
            "Attention expects [N,L,H,D], [N,S,H,D], [N,S,H,D]; got queries={q_dims:?}, keys={k_dims:?}, values={v_dims:?}"
        )));
    }
    if q_dims[0] != k_dims[0] || q_dims[0] != v_dims[0] {
        return Err(LoftrError::InvalidConfig(format!(
            "Attention batch mismatch: queries={}, keys={}, values={}",
            q_dims[0], k_dims[0], v_dims[0]
        )));
    }
    if k_dims[1] != v_dims[1] || k_dims[2] != v_dims[2] || k_dims[3] != v_dims[3] {
        return Err(LoftrError::InvalidConfig(format!(
            "Attention key/value mismatch: keys={k_dims:?}, values={v_dims:?}"
        )));
    }
    if q_dims[2] != k_dims[2] || q_dims[3] != k_dims[3] {
        return Err(LoftrError::InvalidConfig(format!(
            "Attention query/key head mismatch: queries={q_dims:?}, keys={k_dims:?}"
        )));
    }
    Ok(())
}

fn expand_attention_mask(mask: &Tensor, like: &Tensor) -> Result<Tensor, LoftrError> {
    let dims = mask.size();
    let expected = [like.size()[0], like.size()[1]];
    if dims != expected {
        return Err(LoftrError::InvalidConfig(format!(
            "Attention mask expects {expected:?}; got {dims:?}"
        )));
    }
    Ok(mask
        .f_to_device(like.device())?
        .f_to_kind(like.kind())?
        .unsqueeze(-1)
        .unsqueeze(-1))
}

fn expand_full_mask(mask: &Tensor, like: &Tensor, is_query: bool) -> Result<Tensor, LoftrError> {
    let dims = mask.size();
    let expected = [like.size()[0], like.size()[1]];
    if dims != expected {
        return Err(LoftrError::InvalidConfig(format!(
            "Attention mask expects {expected:?}; got {dims:?}"
        )));
    }
    let mask = mask.f_to_device(like.device())?.f_to_kind(Kind::Bool)?;
    Ok(if is_query {
        mask.unsqueeze(-1).unsqueeze(-1)
    } else {
        mask.unsqueeze(1).unsqueeze(-1)
    })
}

#[cfg(test)]
mod tests;
