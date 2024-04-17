#[cfg(test)]
mod tests {
    use bloomfilter::Bloom;

    #[test]
    fn test_bloom_filter() {
        let num_items = 100000;
        let fp_rate = 0.001;

        let mut bloom = Bloom::new_for_fp_rate(num_items, fp_rate);
        bloom.set(&10);

        assert_eq!(bloom.check(&10), true);
        assert_eq!(bloom.check(&20), false);
    }
}
