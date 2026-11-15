//! A package whose build script checks the job count of the build it runs
//! in.

/// The job count that `jobs.txt` names.
///
/// ```
/// assert!(build_jobs::expected() > 0);
/// ```
pub fn expected() -> u32 {
    include_str!("../jobs.txt").trim().parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_a_job_count() {
        assert!(super::expected() > 0);
    }
}
