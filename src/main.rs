mod pgpass;
use inquire::{Confirm, Select};

use pgpass::PgPass;
use std::fmt::Display;

use anyhow::{Context, Result};
use sqlx::{Connection, postgres::PgConnection};

#[derive(sqlx::FromRow, Debug, Clone)]
struct Sensor {
    id: i32,
    name: String,
    sensor_type: String,
}

impl Display for Sensor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let output = format!(
            "{} (ID: {}, TYPE: {})",
            self.name, self.id, self.sensor_type
        );
        write!(f, "{}", output)
    }
}

fn sensor_type_to_data_tables(sensor_type: &str) -> Option<Vec<&'static str>> {
    match sensor_type {
        "COUNT" => Some(vec!["count_data"]),
        "DWELL" => Some(vec!["dwell_data"]),
        "GRIDMAP" => Some(vec!["gridmap_data"]),
        "CUSTOMER_EXPERIENCE" => Some(vec!["customer_experience_data", "count_data"]),
        "ANPR" => Some(vec![
            "anpr_data",
            "odtravel_data",
            "odcount_data",
            "count_data",
        ]),
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let pgpass = PgPass::load()?;

    let credentials = Select::new("Database:", pgpass.all().to_vec()).prompt()?;

    let mut conn = PgConnection::connect(&credentials.to_url()).await?;

    let sensors = sqlx::query_as::<_, Sensor>("SELECT id, name, sensor_type::TEXT FROM sensor")
        .fetch_all(&mut conn)
        .await?;

    let from = Select::new("Select a sensor:", sensors.clone()).prompt()?;

    let same_type_sensors: Vec<&Sensor> = sensors
        .iter()
        .filter(|sensor| sensor.sensor_type == from.sensor_type && sensor.id != from.id)
        .collect();

    if same_type_sensors.is_empty() {
        println!("No other sensors found");
        return Ok(());
    }

    let to = Select::new("Move all data to:", same_type_sensors).prompt()?;

    let confirm_message = format!(
        "Are you certain? This will move all data from {} to {}",
        from, to
    );

    let confirm = Confirm::new(&confirm_message)
        .with_default(false)
        .prompt()?;

    if !confirm {
        return Ok(());
    }

    let data_tables = sensor_type_to_data_tables(&from.sensor_type).with_context(|| {
        format!(
            "Found no applicable tables for sensor type {}",
            from.sensor_type
        )
    })?;

    for table in data_tables {
        let message = format!("Move data from '{}'?", table);
        let confirm_table = Confirm::new(&message).with_default(true).prompt()?;

        if !confirm_table {
            continue;
        }

        let query = if table.contains("odcount_data") || table.contains("odtravel_data") {
            format!(
                r#"
                UPDATE {table}
                SET
                    origin_sensor_id = CASE
                        WHEN origin_sensor_id = $1 THEN $2
                        ELSE origin_sensor_id
                    END,
                    destination_sensor_id = CASE
                        WHEN destination_sensor_id = $1 THEN $2
                        ELSE destination_sensor_id
                    END
                WHERE origin_sensor_id = $1
                OR destination_sensor_id = $1
                "#
            )
        } else {
            format!(
                r#"
                UPDATE {table}
                SET sensor_id = $1
                WHERE sensor_id = $2
                "#
            )
        };

        sqlx::query(sqlx::AssertSqlSafe(query))
            .bind(to.id)
            .bind(from.id)
            .execute(&mut conn)
            .await?;

        println!("Successfully moved data to {to} in '{table}'");
    }

    let delete_sensor = Confirm::new("Delete sensor?").with_default(true).prompt()?;

    if delete_sensor {
        sqlx::query("DELETE FROM sensor WHERE id = $1")
            .bind(from.id)
            .execute(&mut conn)
            .await?;

        println!("Successfully deleted old sensor");
    }

    println!("You may need to refresh continous aggregates depending on the moved data");

    Ok(())
}
