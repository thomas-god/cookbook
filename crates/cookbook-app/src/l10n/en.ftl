app_name = Cookbook

language_label = Language
language_apply = Apply

auth_login_heading = Sign in
auth_register_heading = Create an account
auth_email = Email
auth_username = Username (optional)
auth_password = Password
auth_password_confirm = Confirm password
auth_login_submit = Sign in
auth_register_submit = Create account
auth_logout = Sign out
auth_login_link = Sign in
auth_register_link = Create account
auth_no_account = Don't have an account?
auth_have_account = Already have an account?
auth_invalid_credentials = Invalid email or password.
auth_form_invalid = Please fill in the form correctly.
auth_invalid_email = Enter a valid email address.
auth_password_mismatch = The passwords do not match.
auth_email_taken = An account with this email already exists.
auth_registration_disabled = Registration is disabled.

account_heading = Account
account_menu_label = Account menu
account_email_heading = Email address
account_current_email = Current email
account_new_email = New email
account_username_heading = Username
account_current_username = Current username
account_new_username = New username (optional)
account_username_none = Not set
account_password_heading = Password
account_username_hint = Shown on recipes and in the header. Leave empty to display your email instead.
account_current_password = Current password
account_new_password = New password
account_save_email = Update email
account_save_username = Update username
account_save_password = Update password
account_invalid_current_password = Current password is incorrect.
account_updated_email = Your email address was updated.
account_updated_username = Your username was updated.
account_updated_password = Your password was updated.
account_danger_heading = Danger zone
account_delete_hint = Deleting your account is permanent. Recipes you created will be kept.
account_delete_confirm_label = Type your email address to confirm
account_delete_submit = Delete my account
account_delete_confirm = Delete your account? This cannot be undone.
account_delete_error_email_mismatch = The email address does not match your account.
account_delete_error_last_admin = You cannot delete the last administrator account.
user_deleted = Deleted user

admin_users_link = Users
admin_users_heading = Users
admin_users_new = Add user
admin_users_empty = No users yet.
admin_users_back = Back to users
admin_user_created = User created.
admin_user_deleted = User deleted.
admin_user_updated_role = Role updated.
admin_user_last_admin = You cannot remove the last administrator.
admin_user_you = (you)
admin_user_edit = Edit
admin_user_promote = Make administrator
admin_user_demote = Remove administrator
admin_user_danger_heading = Danger zone
admin_user_delete_hint = Deleting this user is permanent. Recipes they created will be kept.
admin_user_delete_submit = Delete user
admin_user_delete_confirm = Delete this user? This cannot be undone.
admin_user_create_heading = Add user
admin_user_create_as_admin = Create as administrator
admin_user_create_submit = Create user
admin_user_edit_heading = Edit user
admin_user_role_heading = Role
admin_user_current_role = Current role
admin_role_admin = Administrator
admin_role_user = User

recipe = Recipe
new_recipe = New recipe
add_recipe_new_hint = Start from scratch.
add_recipe_import = Import recipe
add_recipe_import_hint = From photos or a PDF.
recipes_list_heading = Recipes
recipes_empty = No recipes yet.
recipe_search_placeholder = Title, ingredient, step...
recipe_search_empty = No recipes match your search.
recipe_search_categories = Categories
recipe_search_exclude_needs_review = Hide unverified recipes
recipe_search_favorites_only = Favorites only
recipe_search_pagination = Search results pages
recipe_search_page = Page { $current } of { $total }
recipe_search_previous = Previous
recipe_search_next = Next
recipe_back_to_list = Back to recipes
recipe_author = Author
recipe_total_duration = Total time
recipe_minutes = minutes
recipe_servings = Servings
recipe_cooked_count = { $count ->
    [one] Cooked once
   *[other] Cooked { $count } times
}
recipe_parts = Parts
category_label = Category
category_none = Not specified
category_aperitif = Aperitif
category_starter = Starter
category_main = Main
category_side = Side
category_dessert = Dessert
category_breakfast = Breakfast
category_snack = Snack
category_drink = Drink
category_other = Other
scope_heading = Sharing
scope_label = Who can access this recipe
scope_private = Private
scope_viewable = Viewable
scope_editable = Editable
scope_hint = Private recipes are only visible to you. Viewable recipes can be read by anyone signed in, but only you can edit them. Editable recipes can be read and edited by anyone signed in.
scope_save = Save sharing settings
serving_person = { $count } person
serving_people = { $count } people
recipe_ingredients = Ingredients
recipe_steps = Steps
recipe_step = Step
recipe_cook = Cook mode
cooked_event_heading = Cooking log
cooked_event_hint = Keep track of when you cooked this recipe and how it went.
cooked_event_new = New cook entry
cooked_event_empty = You have not cooked this recipe yet.
cooked_event_date = Date cooked
cooked_event_date_display = { $month ->
    [1] Jan. { $day }, { $year }
    [2] Feb. { $day }, { $year }
    [3] Mar. { $day }, { $year }
    [4] Apr. { $day }, { $year }
    [5] May { $day }, { $year }
    [6] Jun. { $day }, { $year }
    [7] Jul. { $day }, { $year }
    [8] Aug. { $day }, { $year }
    [9] Sep. { $day }, { $year }
   [10] Oct. { $day }, { $year }
   [11] Nov. { $day }, { $year }
   [12] Dec. { $day }, { $year }
   *[other] { $day }/{ $month }/{ $year }
}
cooked_event_note = Note (optional)
cooked_event_note_placeholder = How did it turn out? Any changes you made?
cooked_event_add = Log this cook
cooked_event_edit = Edit
cooked_event_save = Save
cooked_event_delete = Delete
cooked_event_delete_confirm = Delete this cooking log entry? This cannot be undone.
cooked_event_log_heading = Cooking log
cooked_event_log_hint = Every recipe you have cooked, newest first.
cooked_event_log_empty = You have not logged any cooked recipe yet.
cooked_event_log_link = Cooking log
cooked_event_log_pagination = Cooking log pages
cooked_event_recipe_deleted = Recipe deleted
cooked_event_recipe_unavailable = Recipe unavailable
cooked_event_page = Page { $current } of { $total }
cooked_event_previous = Previous
cooked_event_next = Next
cooked_event_invalid_form = Enter a valid date.
cooked_event_note_too_long = The note is too long (maximum 2000 characters).
cook_exit = Back to recipe
cook_no_ingredients = No ingredients listed.
cook_close_ingredients = Close ingredients
cook_text_smaller = Decrease text size
cook_text_larger = Increase text size
cook_start = Start cooking
cook_no_steps = This recipe has no steps yet.
cook_back_to_ingredients = Ingredients
cook_previous = Previous
cook_next = Next
cook_log_session = Log this cooking session
cook_part_progress = Part { $current } of { $total }
cook_step_progress = Step { $current } of { $total }
recipe_edit = Edit
recipe_verify = Verify
recipe_actions = Recipe actions
recipe_favorite_add = Add to favorites
recipe_favorite_remove = Remove from favorites
recipe_favorite_marker = Favorite
recipe_delete = Delete recipe
recipe_delete_confirm = Delete this recipe? This cannot be undone.
recipe_needs_review = Needs review
recipe_verified = Verified
recipe_needs_review_notice = This recipe was imported from photos and has not been reviewed yet. Check the details, then save to verify it.
recipe_history = History
recipe_back_to_recipe = Back to recipe
recipe_back_to_history = Back to history
recipe_view_current = View current recipe
recipe_history_heading = History
recipe_history_empty = No revisions yet.
recipe_history_created = Created the recipe
recipe_history_imported = Imported from photos
recipe_history_edited = Edited the recipe
recipe_history_converted_to_simple = Converted to simple recipe
recipe_history_converted_to_multipart = Converted to multipart recipe
recipe_history_restored = Restored from { $timestamp }
recipe_history_restored_fallback = Restored a previous revision
recipe_restore_revision = Restore this revision
recipe_restore_revision_hint = Restoring creates a new revision with this content. The existing history is kept.
recipe_history_edited_by = Edited by
recipe_history_view = View
recipe_export = Export PDF
recipe_export_heading = Export recipe as PDF
recipe_export_description = Choose what to include in the PDF.
recipe_export_images = Include recipe images
recipe_export_sources = Include source documents
recipe_export_hide_author = Hide author name
recipe_export_show_timestamp = Show revision timestamp
recipe_export_revision = Revision
recipe_export_revision_info = Exporting revision from { $timestamp }
recipe_export_submit = Download PDF
recipe_export_failed = The PDF could not be generated. Please try again.
recipe_export_back = Back to recipe
recipe_conflict = This recipe was changed by someone else. Reload the form and try again.
recipe_import_heading = Add a recipe from photos or a PDF
recipe_import_images = Photos or PDF
recipe_import_hint = Select photos of a handwritten recipe, cookbook page, or a PDF of an existing copy. A recipe will be created from the document, and you will be able to verify and review it.
recipe_import_submit = Create recipe from document
recipe_import_working = Importing document. This can take a moment.
recipe_import_empty = Select at least one photo or PDF.
recipe_import_unsupported = Only JPEG, PNG, and PDF documents are supported.
recipe_import_too_many_pages = PDFs with more than 10 pages are not supported.
recipe_import_mixed_documents = Import a PDF on its own, without photos or other PDFs.
recipe_import_failed = The document could not be turned into a recipe. Check the vision configuration and try again.
recipe_import_save_failed = The imported recipe could not be saved. Please try again.
recipe_import_retry = Your document was not kept after the error. Select it again to retry.

source_documents_heading = Source recipe
source_documents_empty = No source document yet.
source_documents_upload = Add source documents
source_documents_upload_hint = These are reference photos or PDFs for the recipe, separate from the recipe's presentation images.
source_documents_upload_submit = Upload documents
source_documents_delete = Delete
source_documents_unsupported = Only JPEG, PNG, and PDF documents are supported.
source_documents_upload_empty = Select at least one photo or PDF.
source_document_alt = Recipe source document
source_document_pdf_open = Open PDF
recipe_images_heading = Photos
recipe_image_alt = Recipe photo
recipe_image_invalid_target = An image is attached to a step that no longer exists. Reload the form and try again.

form_add_recipe = Add a recipe
form_new_recipe = New recipe
form_recipe_images = Recipe photos
form_recipe_images_empty = No recipe photos yet.
form_recipe_images_remove = Remove this photo
form_recipe_images_upload = Add recipe photos
form_recipe_images_upload_hint = These photos illustrate the finished recipe.
form_step_images = Step photos
form_step_images_upload_hint = Photos showing this step in progress or its result.
form_edit_recipe = Edit recipe
form_verify_recipe = Verify recipe
form_save_recipe = Save changes
form_verify_and_save = Verify and save
form_edit_message = Edit note (optional)
    .placeholder = Reduce the sugar, use less salt, ...
form_verification_message = Verification note (optional)
    .placeholder = Checked the quantities, corrected the oven temperature, ...
form_cancel = Cancel
form_timing = Timing (minutes)
form_revision_note = Revision note
form_edit_field = Edit { $field }
form_done_editing = Done editing { $field }
form_remove_field = Remove { $field }
form_remove_part = Remove part
form_not_set = Not set

form_title = Title
    .placeholder = Grandma's apple pie
form_description = Description (optional)
    .placeholder = A short description of your recipe.
form_servings = Servings
form_servings_type = Servings type
form_servings_none = Not specified
form_servings_people = People
form_servings_portions = Portions
form_servings_count = Count
form_servings_label = Label (optional)
    .placeholder = cookies, slices, ...
form_preparation = Preparation
form_rest = Rest
form_cooking = Cooking

form_ingredients = Ingredients
form_add_ingredient = Add ingredient
form_ingredient = Ingredient
    .placeholder = Flour
form_amount_type = Amount type
form_amount_measured = Measured
form_amount_text = Free text
form_amount_text_value = Amount (free text)
    .placeholder = a pinch, salt to taste, ...
form_quantity = Quantity
    .placeholder = 200
form_unit = Unit
form_unit_other = Custom unit
    .placeholder = sachet, bunch, pinch

unit_gram = Gram
unit_kilogram = Kilogram
unit_millilitre = Millilitre
unit_centilitre = Centilitre
unit_litre = Litre
unit_piece = Piece
unit_teaspoon = Teaspoon
unit_tablespoon = Tablespoon
unit_other = Other

quantity_unit_gram = { $count ->
    [one] g
   *[other] g
}
quantity_unit_kilogram = { $count ->
    [one] kg
   *[other] kg
}
quantity_unit_millilitre = { $count ->
    [one] mL
   *[other] mL
}
quantity_unit_centilitre = { $count ->
    [one] cL
   *[other] cL
}
quantity_unit_litre = { $count ->
    [one] L
   *[other] L
}
quantity_unit_teaspoon = { $count ->
    [one] Teaspoon
   *[other] Teaspoons
}
quantity_unit_tablespoon = { $count ->
    [one] Tablespoon
   *[other] Tablespoons
}

form_steps = Steps
form_add_step = Add step
form_insert_step_below = Add step below
form_move_step_up = Move step up
form_move_step_down = Move step down
form_split_step = Duplicate step
form_step_title = Step title (optional)
    .placeholder = Make the dough
form_instruction = Instruction
    .placeholder = Mix the flour and butter until crumbly.

form_part_heading = Part { $count }
form_add_part = Add part
form_part_title = Part title
    .placeholder = Pastry cream
form_part_description = Part description (optional)
form_insert_part_below = Add part below
form_move_part_up = Move part up
form_move_part_down = Move part down

recipe_ingredient_conflicts_notice = This recipe has ingredients with conflicting quantities. Review the duplicated ingredient and adjust them when editing.

form_convert_to_multipart = Convert to a multipart recipe
form_convert_to_simple = Merge parts into a simple recipe
form_recipe_type = Recipe type
form_recipe_type_simple = Simple recipe
form_recipe_type_multipart = Multipart recipe
form_new_multipart_recipe = New multipart recipe
form_new_simple_recipe = New simple recipe
