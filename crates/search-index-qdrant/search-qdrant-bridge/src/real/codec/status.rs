const fn strong_ordering() -> WriteOrdering {
    WriteOrdering {
        r#type: WriteOrderingType::Strong as i32,
    }
}

fn update_completed(status: i32) -> bool {
    UpdateStatus::try_from(status)
        .is_ok_and(|parsed| parsed == UpdateStatus::Completed)
}
