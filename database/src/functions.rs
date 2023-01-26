use diesel::{
	sql_function,
	sql_types::{Text, Timestamptz},
};

sql_function!(fn date_trunc(field: Text, timestamp: Timestamptz) -> Timestamptz);
