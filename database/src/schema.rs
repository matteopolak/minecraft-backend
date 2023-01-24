use diesel::{allow_tables_to_appear_in_same_query, table};

table! {
	names (username) {
		username -> Text,
		popularity -> Float8,
		available -> Bool,
		#[sql_name = "createdAt"]
		created_at -> Timestamptz,
		#[sql_name = "updatedAt"]
		updated_at -> Timestamptz,
		#[sql_name = "checkedAt"]
		checked_at -> Timestamptz,
		valid -> Nullable<Bool>,
		#[sql_name = "verifiedAt"]
		verified_at -> Timestamptz,
		definition -> Nullable<Array<Text>>,
		frequency -> Float8,
		length -> Integer,
		updating -> Bool,
		tags -> Nullable<Array<Text>>,
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
	#[sql_name = "_NameToUser"]
	likes (username, user_id) {
		#[sql_name = "A"]
		username -> Text,
		#[sql_name = "B"]
		user_id -> Integer,
	}
}

allow_tables_to_appear_in_same_query!(names, users, proxies, accounts, likes);
