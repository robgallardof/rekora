namespace Shop.Orders;

public record OrderLine(string Sku, int Quantity, decimal UnitPrice);

public record BusinessScope(Guid TenantId);

public interface IOrderRepository
{
    Task<Order> GetAsync(Guid id, BusinessScope scope);
    Task SaveAsync(Order order, byte[] rowVersion);
}

public class Order
{
    public Guid Id { get; init; }
    public byte[] RowVersion { get; set; } = Array.Empty<byte>();
    public List<OrderLine> Lines { get; } = new();

    public void ReplaceLine(OrderLine line)
    {
        Lines.RemoveAll(l => l.Sku == line.Sku);
        Lines.Add(line);
    }
}
