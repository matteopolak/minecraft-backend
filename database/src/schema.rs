use diesel::{allow_tables_to_appear_in_same_query, table};

table! {
	names (username) {
		username -> Text,
		popularity -> Float8,
		#[sql_name = "createdAt"]
		created_at -> Timestamptz,
		#[sql_name = "updatedAt"]
		updated_at -> Timestamptz,
		#[sql_name = "checkedAt"]
		checked_at -> Timestamptz,
		#[sql_name = "verifiedAt"]
		verified_at -> Timestamptz,
		definition -> Nullable<Array<Text>>,
		frequency -> Float8,
		length -> Integer,
		updating -> Bool,
		tags -> Nullable<Array<Text>>,
		status -> SmallInt,
	}
}

table! {
	users (id) {
		id -> Integer,
		key -> Text,
	}
}

table! {
	proxies (id) {
		id -> Integer,
		address -> Text,
		port -> Integer,
		username -> Nullable<Text>,
		password -> Nullable<Text>,
		note -> Nullable<Text>,
	}
}

table! {
	accounts (id) {
		id -> Integer,
		username -> Text,
		password -> Text,
	}
}

table! {
	snipes (username) {
		username -> Text,
		#[sql_name = "createdAt"]
		created_at -> Timestamptz,
		needed -> SmallInt,
		count -> SmallInt,
		email -> Text,
		password -> Text,
	}
}

table! {
	#[sql_name = "_NameToUser"]
	likes (username, user_id) {
		#[sql_name = "A"]
		username -> Text,
		#[sql_name = "B"]
		user_id -> Integer,
	}
}

allow_tables_to_appear_in_same_query!(names, users, proxies, accounts, likes);
