package models

import (
	"time"

	"github.com/google/uuid"
)

type Invoice struct {
	ID        uuid.UUID `gorm:"type:uuid;primaryKey"`
	AccountID uuid.UUID `gorm:"index:idx_invoices_account_status_created_at,priority:1"`
	Status    string    `gorm:"index:idx_invoices_account_status_created_at,priority:2"`
	CreatedAt time.Time `gorm:"index:idx_invoices_account_status_created_at,priority:3"`
}
