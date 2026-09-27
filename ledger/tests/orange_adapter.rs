#[allow(dead_code)]
mod common;

use anbarr_ledger::money::Currency;
use anbarr_ledger::reconcile::{OrangeMoneySource, Rail, RailStatementSource};
use chrono::{TimeZone, Utc};

#[tokio::test]
async fn orange_money_source_maps_statement_lines_for_a_window() {
    let since = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let until = Utc.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();

    let source = RailStatementSource::OrangeMoney {
        source: OrangeMoneySource::new(vec![
            anbarr_ledger::reconcile::OrangeMoneyStatementLine {
                external_reference: "ref-001".into(),
                amount_minor: 10_000,
                currency: Currency::Sle,
                occurred_at: Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
            },
            anbarr_ledger::reconcile::OrangeMoneyStatementLine {
                external_reference: "ref-002".into(),
                amount_minor: -2_000,
                currency: Currency::Sle,
                occurred_at: Utc.with_ymd_and_hms(2026, 1, 1, 18, 0, 0).unwrap(),
            },
            anbarr_ledger::reconcile::OrangeMoneyStatementLine {
                external_reference: "ref-003".into(),
                amount_minor: 5_000,
                currency: Currency::Usd,
                occurred_at: Utc.with_ymd_and_hms(2026, 1, 1, 22, 0, 0).unwrap(),
            },
        ]),
    };

    let lines = source
        .fetch(Rail::OrangeMoney, Currency::Sle, since, until)
        .await
        .expect("orange money source should map lines");

    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].external_reference, "ref-001");
    assert_eq!(lines[0].amount_minor, 10_000);
    assert_eq!(lines[1].amount_minor, -2_000);
    assert!(lines.iter().all(|line| line.currency == Currency::Sle));
}
